use std::fmt::Debug;

use error_stack::{Report, ResultExt};
use serde::Deserialize;
use tracing::{debug, trace};

use crate::{
    fields,
    manifest::dependency::RegistrySource,
    updater::{CheckForUpdateError, UpdateResult},
};

#[derive(Debug, Clone)]
pub struct Updater {
    client: reqwest::Client,
}

impl Updater {
    pub const fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    #[tracing::instrument(skip(self), fields(url))]
    pub async fn get_versions(
        &self,
        name: &str,
    ) -> Result<Vec<CrateVersion>, Report<CheckForUpdateError>> {
        let url = format!("https://crates.io/api/v1/crates/{name}/versions");

        fields!(url = &url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .change_context(CheckForUpdateError::RequestFailed)?
            .json::<CratesIoVersionsResponse>()
            .await
            .change_context(CheckForUpdateError::DeserializationFailed)?;

        Ok(response.versions)
    }

    #[tracing::instrument(skip(versions, version), fields(current_version = %version, version))]
    pub fn filter_update_results<'a>(
        version: &RegistrySource,
        versions: impl Iterator<Item = &'a CrateVersion> + Debug,
    ) -> Result<UpdateResult<semver::Version>, Report<CheckForUpdateError>> {
        let mut latest_version = None;
        let mut semantic_latest_version = None;

        let compatibility_requirement = version.compatible_req();

        for v in versions {
            trace!(version = %&v.num, "Checking compatibility for version");
            if v.yanked {
                trace!(version = %&v.num, "Version is yanked");
                continue;
            }

            if let Some(ref current) = latest_version
                && v.num <= *current
            {
                trace!(version = %&v.num, "Version is not newer than current latest");
                continue;
            }

            debug!(version = %&v.num, "Version is newer than current latest");
            latest_version = Some(v.num.clone());

            if compatibility_requirement.matches(&v.num) {
                if semantic_latest_version
                    .as_ref()
                    .is_some_and(|c| &v.num <= c)
                {
                    trace!(version = %&v.num, "Version is not newer than current semantic latest");
                    continue;
                }

                debug!(version = %&v.num, "Version is compatible and newer than current semantic latest");
                semantic_latest_version = Some(v.num.clone());
            }
        }

        debug!(latest_version = ?latest_version, semantic_latest_version = ?semantic_latest_version, "Filtered update results");

        Ok(UpdateResult {
            latest_version,
            semantic_latest_version,
        })
    }
}

#[derive(Debug, Deserialize)]
struct CratesIoVersionsResponse {
    pub versions: Vec<CrateVersion>,
}

#[derive(Debug, Deserialize)]
pub struct CrateVersion {
    pub num: semver::Version,
    pub yanked: bool,
}
