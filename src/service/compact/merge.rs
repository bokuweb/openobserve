// Copyright 2023 Zinc Labs Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::{collections::HashMap, io::Write, sync::Arc};

use ::datafusion::{arrow::datatypes::Schema, common::FileType, error::DataFusionError};
use ahash::AHashMap;
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use config::{
    ider,
    meta::stream::{FileKey, FileMeta, StreamType},
    metrics,
    utils::parquet::parse_file_key_columns,
    CONFIG, FILE_EXT_PARQUET,
};
use tokio::{sync::Semaphore, task::JoinHandle};

use crate::{
    common::{
        infra::{cache, file_list as infra_file_list, storage},
        meta::stream::{PartitionTimeLevel, StreamStats},
        utils::json,
    },
    service::{db, file_list, search::datafusion, stream},
};

/// compactor run steps on a stream:
/// 3. get a cluster lock for compactor stream
/// 4. read last compacted offset: year/month/day/hour
/// 5. read current hour all files
/// 6. compact small files to big files -> COMPACTOR_MAX_FILE_SIZE
/// 7. write to storage
/// 8. delete small files keys & write big files keys, use transaction
/// 9. delete small files from storage
/// 10. update last compacted offset
/// 11. release cluster lock
pub async fn merge_by_stream(
    org_id: &str,
    stream_name: &str,
    stream_type: StreamType,
) -> Result<(), anyhow::Error> {
    let start = std::time::Instant::now();

    // get last compacted offset
    let (mut offset, _node) =
        db::compact::files::get_offset(org_id, stream_name, stream_type).await;

    // get schema
    let mut schema = db::schema::get(org_id, stream_name, stream_type).await?;
    let stream_settings = stream::stream_settings(&schema).unwrap_or_default();
    let partition_time_level =
        stream::unwrap_partition_time_level(stream_settings.partition_time_level, stream_type);
    let stream_created = stream::stream_created(&schema).unwrap_or_default();
    std::mem::take(&mut schema.metadata);
    let schema = Arc::new(schema);
    if offset == 0 {
        offset = stream_created
    }
    if offset == 0 {
        return Ok(()); // no data
    }
    let offset = offset;
    let offset_time: DateTime<Utc> = Utc.timestamp_nanos(offset * 1000);
    let offset_time_hour = Utc
        .with_ymd_and_hms(
            offset_time.year(),
            offset_time.month(),
            offset_time.day(),
            offset_time.hour(),
            0,
            0,
        )
        .unwrap()
        .timestamp_micros();
    let offset_time_day = Utc
        .with_ymd_and_hms(
            offset_time.year(),
            offset_time.month(),
            offset_time.day(),
            0,
            0,
            0,
        )
        .unwrap()
        .timestamp_micros();

    // check offset
    let time_now: DateTime<Utc> = Utc::now();
    let time_now_hour = Utc
        .with_ymd_and_hms(
            time_now.year(),
            time_now.month(),
            time_now.day(),
            time_now.hour(),
            0,
            0,
        )
        .unwrap()
        .timestamp_micros();
    // 1. if step_secs less than 1 hour, must wait for at least max_file_retention_time
    // 2. if step_secs greater than 1 hour, must wait for at least 3 * max_file_retention_time
    // -- first period: the last hour local file upload to storage, write file list
    // -- second period, the last hour file list upload to storage
    // -- third period, we can do the merge, so, at least 3 times of
    // max_file_retention_time
    if (CONFIG.compact.step_secs < 3600
        && time_now.timestamp_micros() - offset
            <= Duration::seconds(CONFIG.limit.max_file_retention_time as i64)
                .num_microseconds()
                .unwrap())
        || (CONFIG.compact.step_secs >= 3600
            && (offset >= time_now_hour
                || time_now.timestamp_micros() - time_now_hour
                    <= Duration::seconds(CONFIG.limit.max_file_retention_time as i64)
                        .num_microseconds()
                        .unwrap()
                        * 3))
    {
        return Ok(()); // the time is future, just wait
    }

    // get current hour(day) all files
    let (partition_offset_start, partition_offset_end) =
        if partition_time_level == PartitionTimeLevel::Daily {
            (
                offset_time_day,
                offset_time_day + Duration::hours(24).num_microseconds().unwrap() - 1,
            )
        } else {
            (
                offset_time_hour,
                offset_time_hour + Duration::hours(1).num_microseconds().unwrap() - 1,
            )
        };
    let files = file_list::query(
        org_id,
        stream_name,
        stream_type,
        partition_time_level,
        partition_offset_start,
        partition_offset_end,
        true,
    )
    .await
    .map_err(|e| anyhow::anyhow!("query file list failed: {}", e))?;

    if files.is_empty() {
        // this hour is no data, and check if pass allowed_upto, then just write new
        // offset if offset > 0 && offset_time_hour +
        // Duration::hours(CONFIG.limit.allowed_upto).num_microseconds().unwrap() <
        // time_now_hour { -- no check it
        // }
        let offset = offset
            + Duration::seconds(CONFIG.compact.step_secs)
                .num_microseconds()
                .unwrap();
        db::compact::files::set_offset(org_id, stream_name, stream_type, offset).await?;
        return Ok(());
    }

    // do partition by partition key
    let mut partition_files_with_size: HashMap<String, Vec<FileKey>> = HashMap::default();
    for file in files {
        let file_name = file.key.clone();
        let prefix = file_name[..file_name.rfind('/').unwrap()].to_string();
        let partition = partition_files_with_size.entry(prefix).or_default();
        partition.push(file.to_owned());
    }

    // collect stream stats
    let mut stream_stats = StreamStats::default();

    // use mutiple threads to merge
    let semaphore = std::sync::Arc::new(Semaphore::new(CONFIG.limit.file_move_thread_num));
    let mut tasks = Vec::with_capacity(partition_files_with_size.len());
    for (prefix, files_with_size) in partition_files_with_size.into_iter() {
        let org_id = org_id.to_string();
        let stream_name = stream_name.to_string();
        let schema = schema.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let task: JoinHandle<Result<(), anyhow::Error>> = tokio::task::spawn(async move {
            let mut files_with_size = files_with_size.to_owned();
            // sort by file size
            files_with_size.sort_by(|a, b| a.meta.original_size.cmp(&b.meta.original_size));
            // delete duplicated files
            files_with_size.dedup_by(|a, b| a.key == b.key);
            loop {
                // yield to other tasks
                tokio::task::yield_now().await;
                // merge file and get the big file key
                let (new_file_name, new_file_meta, new_file_list) = match merge_files(
                    &org_id,
                    &stream_name,
                    stream_type,
                    schema.clone(),
                    &prefix,
                    &files_with_size,
                )
                .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("[COMPACT] merge files failed: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }
                };
                if new_file_name.is_empty() {
                    if CONFIG.common.print_key_event {
                        log::info!(
                            "[COMPACTOR] processing merge for {org_id}/{stream_type}/{stream_name}, new merge file is empty, new_file_list is {}",
                            new_file_list.len()
                        );
                    }
                    if new_file_list.is_empty() {
                        // no file need to merge
                        break;
                    } else {
                        // delete files from file_list and continue
                        files_with_size.retain(|f| !&new_file_list.contains(f));
                        continue;
                    }
                }

                // delete small files keys & write big files keys, use transaction
                let mut events = Vec::with_capacity(new_file_list.len() + 1);
                events.push(FileKey {
                    key: new_file_name.clone(),
                    meta: new_file_meta,
                    deleted: false,
                });
                for file in new_file_list.iter() {
                    stream_stats = stream_stats - file.meta.clone();
                    events.push(FileKey {
                        key: file.key.clone(),
                        meta: FileMeta::default(),
                        deleted: true,
                    });
                }
                events.sort_by(|a, b| a.key.cmp(&b.key));

                // write file list to storage
                match write_file_list(&org_id, &events).await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("[COMPACT] write file list failed: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }
                }

                // No need delete file here, will use another task to delete files

                // delete files from file list
                files_with_size.retain(|f| !&new_file_list.contains(f));
            }
            drop(permit);
            Ok(())
        });
        tasks.push(task);
    }

    for task in tasks {
        task.await??;
    }

    // write new offset
    let offset = offset
        + Duration::seconds(CONFIG.compact.step_secs)
            .num_microseconds()
            .unwrap();
    db::compact::files::set_offset(org_id, stream_name, stream_type, offset).await?;

    // update stream stats
    if stream_stats.doc_num != 0 {
        infra_file_list::set_stream_stats(
            org_id,
            &[(
                format!("{org_id}/{stream_type}/{stream_name}"),
                stream_stats,
            )],
        )
        .await?;
    }

    // metrics
    let time = start.elapsed().as_secs_f64();
    metrics::COMPACT_USED_TIME
        .with_label_values(&[org_id, stream_name, stream_type.to_string().as_str()])
        .inc_by(time);
    metrics::COMPACT_DELAY_HOURS
        .with_label_values(&[org_id, stream_name, stream_type.to_string().as_str()])
        .set((time_now_hour - offset_time_hour) / Duration::hours(1).num_microseconds().unwrap());

    Ok(())
}

/// merge some small files into one big file, upload to storage, returns the big
/// file key and merged files
async fn merge_files(
    org_id: &str,
    stream_name: &str,
    stream_type: StreamType,
    schema: Arc<Schema>,
    prefix: &str,
    files_with_size: &[FileKey],
) -> Result<(String, FileMeta, Vec<FileKey>), anyhow::Error> {
    if files_with_size.len() <= 1 {
        return Ok((String::from(""), FileMeta::default(), Vec::new()));
    }

    let mut new_file_size = 0;
    let mut new_file_list = Vec::new();
    let mut deleted_files = Vec::new();
    for file in files_with_size.iter() {
        if new_file_size + file.meta.original_size > CONFIG.compact.max_file_size as i64 {
            break;
        }
        new_file_size += file.meta.original_size;
        new_file_list.push(file.to_owned());
        log::info!("[COMPACT] merge small file: {}", &file.key);
        // metrics
        metrics::COMPACT_MERGED_FILES
            .with_label_values(&[org_id, stream_name, stream_type.to_string().as_str()])
            .inc();
        metrics::COMPACT_MERGED_BYTES
            .with_label_values(&[org_id, stream_name, stream_type.to_string().as_str()])
            .inc_by(file.meta.original_size as u64);
    }
    // no files need to merge
    if new_file_list.len() <= 1 {
        return Ok((String::from(""), FileMeta::default(), Vec::new()));
    }
    let retain_file_list = new_file_list.clone();

    // write parquet files into tmpfs
    let tmp_dir = cache::tmpfs::Directory::default();
    for file in &new_file_list {
        let data = match storage::get(&file.key).await {
            Ok(body) => body,
            Err(err) => {
                log::error!("[COMPACT] merge small file: {}, err: {}", &file.key, err);
                if err.to_string().to_lowercase().contains("not found") {
                    // delete file from file list
                    if let Err(err) = file_list::delete_parquet_file(&file.key, true).await {
                        log::error!(
                            "[COMPACT] delete file: {}, from file_list err: {}",
                            &file.key,
                            err
                        );
                    }
                }
                deleted_files.push(file.key.clone());
                continue;
            }
        };
        tmp_dir.set(&file.key, data)?;
    }
    if !deleted_files.is_empty() {
        new_file_list.retain(|f| !deleted_files.contains(&f.key));
    }
    if new_file_list.len() <= 1 {
        return Ok((String::from(""), FileMeta::default(), retain_file_list));
    }

    // convert the file to the latest version of schema
    let schema_versions = db::schema::get_versions(org_id, stream_name, stream_type).await?;
    let schema_latest = schema_versions.last().unwrap();
    let schema_latest_id = schema_versions.len() - 1;
    let bloom_filter_fields =
        stream::get_stream_setting_bloom_filter_fields(schema_latest).unwrap();
    if CONFIG.common.widening_schema_evolution && schema_versions.len() > 1 {
        for file in &new_file_list {
            // get the schema version of the file
            let schema_ver_id = match db::schema::filter_schema_version_id(
                &schema_versions,
                file.meta.min_ts,
                file.meta.max_ts,
            ) {
                Some(id) => id,
                None => {
                    log::error!(
                        "[COMPACT] merge small file: {}, schema version not found, min_ts: {}, max_ts: {}",
                        &file.key,
                        file.meta.min_ts,
                        file.meta.max_ts
                    );
                    // HACK: use the latest verion if not found in schema versions
                    schema_latest_id
                }
            };
            if schema_ver_id == schema_latest_id {
                continue;
            }
            // cacluate the diff between latest schema and current schema
            let schema = schema_versions[schema_ver_id]
                .clone()
                .with_metadata(HashMap::new());
            let mut diff_fields = AHashMap::default();
            let cur_fields = schema.fields();
            for field in cur_fields {
                if let Ok(v) = schema_latest.field_with_name(field.name()) {
                    if v.data_type() != field.data_type() {
                        diff_fields.insert(v.name().clone(), v.data_type().clone());
                    }
                }
            }
            if diff_fields.is_empty() {
                continue;
            }

            // do the convert
            let mut buf = Vec::new();
            let file_tmp_dir = cache::tmpfs::Directory::default();
            let file_data = storage::get(&file.key).await?;
            file_tmp_dir.set(&file.key, file_data)?;
            datafusion::exec::convert_parquet_file(
                file_tmp_dir.name(),
                &mut buf,
                Arc::new(schema),
                &bloom_filter_fields,
                diff_fields,
                FileType::PARQUET,
            )
            .await
            .map_err(|e| {
                DataFusionError::Plan(format!("convert_parquet_file {}, err: {}", &file.key, e))
            })?;

            // replace the file in tmpfs
            tmp_dir.set(&file.key, buf.into())?;
        }
    }

    let mut buf = Vec::new();
    let mut new_file_meta = datafusion::exec::merge_parquet_files(
        tmp_dir.name(),
        &mut buf,
        schema,
        &bloom_filter_fields,
        new_file_size,
    )
    .await?;
    new_file_meta.original_size = new_file_size;
    new_file_meta.compressed_size = buf.len() as i64;
    if new_file_meta.records == 0 {
        return Err(anyhow::anyhow!("merge_parquet_files error: records is 0"));
    }

    let id = ider::generate();
    let new_file_key = format!("{prefix}/{id}{}", FILE_EXT_PARQUET);

    log::info!(
        "[COMPACT] merge file succeeded, new file: {}, orginal_size: {}, compressed_size: {}",
        new_file_key,
        new_file_meta.original_size,
        new_file_meta.compressed_size,
    );

    // upload file
    match storage::put(&new_file_key, buf.into()).await {
        Ok(_) => Ok((new_file_key, new_file_meta, retain_file_list)),
        Err(e) => Err(e),
    }
}

async fn write_file_list(org_id: &str, events: &[FileKey]) -> Result<(), anyhow::Error> {
    if events.is_empty() {
        return Ok(());
    }
    if CONFIG.common.meta_store_external {
        write_file_list_db_only(org_id, events).await
    } else {
        write_file_list_s3(org_id, events).await
    }
}

async fn write_file_list_db_only(org_id: &str, events: &[FileKey]) -> Result<(), anyhow::Error> {
    let put_items = events
        .iter()
        .filter(|v| !v.deleted)
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();
    let del_items = events
        .iter()
        .filter(|v| v.deleted)
        .map(|v| v.key.clone())
        .collect::<Vec<_>>();
    // set to external db
    // retry 5 times
    let mut success = false;
    let created_at = Utc::now().timestamp_micros();
    for _ in 0..5 {
        if let Err(e) = infra_file_list::batch_add_deleted(org_id, created_at, &del_items).await {
            log::error!(
                "[COMPACT] batch_add_deleted to external db failed, retrying: {}",
                e
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            continue;
        }
        if let Err(e) = infra_file_list::batch_add(&put_items).await {
            log::error!("[COMPACT] batch_add to external db failed, retrying: {}", e);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            continue;
        }
        if let Err(e) = infra_file_list::batch_remove(&del_items).await {
            log::error!(
                "[COMPACT] batch_delete to external db failed, retrying: {}",
                e
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            continue;
        }
        success = true;
        break;
    }
    if !success {
        Err(anyhow::anyhow!("batch_write to external db failed"))
    } else {
        Ok(())
    }
}

async fn write_file_list_s3(org_id: &str, events: &[FileKey]) -> Result<(), anyhow::Error> {
    // write new data into file_list
    let (_stream_key, date_key, _file_name) = parse_file_key_columns(&events.first().unwrap().key)?;
    // upload the new file_list to storage
    let new_file_list_key = format!("file_list/{}/{}.json.zst", date_key, ider::generate());
    let mut buf = zstd::Encoder::new(Vec::new(), 3)?;
    for file in events.iter() {
        let mut write_buf = json::to_vec(&file)?;
        write_buf.push(b'\n');
        buf.write_all(&write_buf)?;
    }
    let compressed_bytes = buf.finish().unwrap();
    storage::put(&new_file_list_key, compressed_bytes.into()).await?;

    // write deleted files into file_list_deleted
    let del_items = events
        .iter()
        .filter(|v| v.deleted)
        .map(|v| v.key.clone())
        .collect::<Vec<_>>();
    if !del_items.is_empty() {
        let deleted_file_list_key = format!(
            "file_list_deleted/{org_id}/{}/{}.json.zst",
            Utc::now().format("%Y/%m/%d/%H"),
            ider::generate()
        );
        let mut buf = zstd::Encoder::new(Vec::new(), 3)?;
        for file in del_items.iter() {
            buf.write_all((file.to_owned() + "\n").as_bytes())?;
        }
        let compressed_bytes = buf.finish().unwrap();
        storage::put(&deleted_file_list_key, compressed_bytes.into()).await?;
    }

    // set to local cache & send broadcast
    // retry 5 times
    for _ in 0..5 {
        // set to local cache
        let mut cache_success = true;
        for event in events.iter() {
            if let Err(e) =
                db::file_list::progress(&event.key, Some(&event.meta), event.deleted, false).await
            {
                cache_success = false;
                log::error!(
                    "[COMPACT] set local cache for file_list failed, retrying: {}",
                    e
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                break;
            }
        }
        if !cache_success {
            continue;
        }
        // send broadcast to other nodes
        if let Err(e) = db::file_list::broadcast::send(events, None).await {
            log::error!(
                "[COMPACT] send broadcast for file_list failed, retrying: {}",
                e
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            continue;
        }
        // broadcast success
        break;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::infra::db as infra_db;

    #[tokio::test]
    async fn test_compact() {
        infra_db::create_table().await.unwrap();
        infra_file_list::create_table().await.unwrap();
        let off_set = Duration::hours(2).num_microseconds().unwrap();
        let _ = db::compact::files::set_offset("nexus", "default", "logs".into(), off_set).await;
        let resp = merge_by_stream("nexus", "default", "logs".into()).await;
        assert!(resp.is_ok());
    }
}
