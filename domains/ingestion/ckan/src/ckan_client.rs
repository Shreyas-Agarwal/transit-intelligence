//! CKAN Action API client — design doc §1. Response shape confirmed against the
//! live API; see the design doc for the full sample payload.

use ti_common::auth::TokenCredentials;
use ti_common::retry::{RetryPolicy, retry_async};

use crate::domain::{UpstreamResource, parse_resource_filename};

#[derive(Debug, thiserror::Error)]
pub enum CkanClientError {
    #[error("request to CKAN API failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("CKAN API returned an error response: {0}")]
    ApiError(String),
    #[error("CKAN API response was not valid JSON in the expected shape: {0}")]
    UnexpectedShape(String),
}

pub struct CkanClient {
    http: reqwest::Client,
    api_url: String,
    dataset_id: String,
    credentials: TokenCredentials,
}

/// Only the fields this client uses. `resources[].name`/`title`/`description`
/// are locale-keyed objects (`{"de": ..., "en": ..., ...}`), not filenames —
/// the actual filename is the last path segment of `url`.
#[derive(serde::Deserialize)]
struct PackageShowResponse {
    success: bool,
    result: Option<PackageShowResult>,
    error: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct PackageShowResult {
    resources: Vec<CkanResource>,
}

#[derive(serde::Deserialize)]
struct CkanResource {
    url: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    last_modified: Option<String>,
    #[serde(default)]
    hash: Option<String>,
}

impl CkanClient {
    pub fn new(
        http: reqwest::Client,
        api_url: String,
        dataset_id: String,
        credentials: TokenCredentials,
    ) -> Self {
        Self {
            http,
            api_url,
            dataset_id,
            credentials,
        }
    }

    /// Fetches the dataset's resource list and returns every entry that parses
    /// as a GTFS-S zip resource. Non-zip resources are skipped.
    pub async fn list_gtfs_zip_resources(&self) -> Result<Vec<UpstreamResource>, CkanClientError> {
        let endpoint = reqwest::Url::parse_with_params(
            &format!("{}/package_show", self.api_url),
            &[("id", self.dataset_id.as_str())],
        )
        .map_err(|e| CkanClientError::UnexpectedShape(format!("invalid CKAN API URL: {e}")))?;

        let response = retry_async(
            RetryPolicy::new(3, std::time::Duration::from_secs(2)),
            || async {
                let request = self.credentials.apply(self.http.get(endpoint.clone()));
                request.send().await?.error_for_status()
            },
        )
        .await?;

        let body: PackageShowResponse = response
            .json()
            .await
            .map_err(|e| CkanClientError::UnexpectedShape(e.to_string()))?;

        if !body.success {
            let error_detail = body
                .error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "unknown error".to_string());
            return Err(CkanClientError::ApiError(error_detail));
        }

        let result = body
            .result
            .ok_or_else(|| CkanClientError::UnexpectedShape("missing result field".to_string()))?;

        let mut resources = Vec::new();
        for resource in result.resources {
            let is_zip = resource
                .format
                .as_deref()
                .is_some_and(|f| f.eq_ignore_ascii_case("zip"))
                || resource.url.to_lowercase().ends_with(".zip");
            if !is_zip {
                continue;
            }

            let filename = resource
                .url
                .rsplit('/')
                .next()
                .unwrap_or(&resource.url)
                .to_string();

            match parse_resource_filename(&filename) {
                Ok((name_prefix, version)) => resources.push(UpstreamResource {
                    version,
                    name_prefix,
                    download_url: resource.url,
                    original_filename: filename,
                    publisher_last_modified: non_empty(resource.last_modified),
                    upstream_hash: non_empty(resource.hash),
                }),
                Err(e) => {
                    tracing::warn!(filename, error = %e, "skipping CKAN resource with unparseable filename");
                }
            }
        }

        Ok(resources)
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}
