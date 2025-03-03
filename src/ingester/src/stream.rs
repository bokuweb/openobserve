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

use std::{path::PathBuf, sync::Arc};

use arrow_schema::Schema;

use crate::{
    entry::{Entry, RecordBatchEntry},
    errors::*,
    partition::Partition,
    rwmap::RwMap,
};

pub(crate) struct Stream {
    partitions: RwMap<Arc<str>, Partition>, // key: schema version hash, val: partitions
}

impl Stream {
    pub(crate) fn new() -> Self {
        Self {
            partitions: RwMap::default(),
        }
    }

    pub(crate) async fn write(&mut self, schema: Arc<Schema>, entry: Entry) -> Result<()> {
        let mut rw = self.partitions.write().await;
        let partition = rw
            .entry(entry.schema_key.clone())
            .or_insert_with(|| Partition::new(schema));
        partition.write(entry).await
    }

    pub(crate) async fn read(
        &self,
        time_range: Option<(i64, i64)>,
    ) -> Result<Vec<(Arc<Schema>, Vec<Arc<RecordBatchEntry>>)>> {
        let r = self.partitions.read().await;
        let mut batches = Vec::with_capacity(r.len());
        for partition in r.values() {
            batches.push(partition.read(time_range).await?);
        }
        Ok(batches)
    }

    pub(crate) async fn persist(
        &self,
        thread_id: usize,
        org_id: &str,
        stream_type: &str,
        stream_name: &str,
    ) -> Result<Vec<(PathBuf, i64)>> {
        let mut paths = Vec::new();
        let r = self.partitions.read().await;
        for (_, partition) in r.iter() {
            paths.extend(
                partition
                    .persist(thread_id, org_id, stream_type, stream_name)
                    .await?,
            );
        }
        Ok(paths)
    }
}
