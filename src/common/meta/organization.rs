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

use config::CONFIG;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{alerts::Alert, functions::Transform, stream::Stream};

pub const DEFAULT_ORG: &str = "default";
pub const CUSTOM: &str = "custom";
pub const THRESHOLD: i64 = 9383939382;

#[derive(Serialize, ToSchema)]
pub struct Organization {
    pub identifier: String,
    pub label: String,
}

#[derive(Serialize, Clone, ToSchema)]
pub struct OrgUser {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
}

#[derive(Serialize, ToSchema)]
pub struct OrgDetails {
    pub id: i64,
    pub identifier: String,
    pub name: String,
    pub user_email: String,
    pub ingest_threshold: i64,
    pub search_threshold: i64,
    #[serde(rename = "type")]
    pub org_type: String,
    #[serde(rename = "UserObj")]
    pub user_obj: OrgUser,
}

#[derive(Serialize, ToSchema)]
pub struct OrganizationResponse {
    pub data: Vec<OrgDetails>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct OrgSummary {
    pub streams: Vec<Stream>,
    pub functions: Vec<Transform>,
    pub alerts: Vec<Alert>,
}

/// A container for passcodes and rumtokens
#[derive(Serialize, ToSchema)]
pub enum IngestionTokensContainer {
    Passcode(IngestionPasscode),
    RumToken(RumIngestionToken),
}

#[derive(Serialize, ToSchema)]
pub struct IngestionPasscode {
    pub passcode: String,
    pub user: String,
}

#[derive(Serialize, ToSchema)]
pub struct PasscodeResponse {
    pub data: IngestionPasscode,
}

#[derive(Serialize, ToSchema)]
pub struct RumIngestionToken {
    pub user: String,
    pub rum_token: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct RumIngestionResponse {
    pub data: RumIngestionToken,
}

fn default_scrape_interval() -> u32 {
    CONFIG.common.default_scrape_interval
}

#[derive(Serialize, ToSchema, Deserialize, Debug, Clone)]
pub struct OrganizationSetting {
    /// Ideally this should be the same as prometheus-scrape-interval (in
    /// seconds).
    #[serde(default = "default_scrape_interval")]
    pub scrape_interval: u32,
}

impl Default for OrganizationSetting {
    fn default() -> Self {
        Self {
            scrape_interval: default_scrape_interval(),
        }
    }
}

#[derive(Serialize, ToSchema, Deserialize, Debug, Clone)]
pub struct OrganizationSettingResponse {
    pub data: OrganizationSetting,
}
