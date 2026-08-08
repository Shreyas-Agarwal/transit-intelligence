//! Streaming download to a staging path with size verification (design doc §5,
//! steps 1–2).

use std::path::Path;

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("io error writing staged download: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "downloaded {actual} bytes but Content-Length header said {expected} bytes \
         (truncated or interrupted transfer)"
    )]
    SizeMismatch { expected: u64, actual: u64 },
}

pub struct DownloadOutcome {
    pub bytes: u64,
    pub sha256: String,
    pub content_length_header: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Streams `url` to `part_path`, then renames it to `final_zip_path` once the
/// transfer completes and its size checks out against the `Content-Length`
/// response header (when the server sent one).
///
/// This happens *before* extraction (design doc §5 step 2: "so we never spend
/// time unzipping something we already know is broken"). On any failure, the
/// partial file at `part_path` is removed so staging never accumulates
/// half-written downloads; the caller doesn't need its own cleanup for this path.
pub async fn download_to_staging(
    http: &reqwest::Client,
    url: &str,
    part_path: &Path,
    final_zip_path: &Path,
) -> Result<DownloadOutcome, DownloadError> {
    if let Some(parent) = part_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let result = try_download(http, url, part_path).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(part_path).await;
    }
    let outcome = result?;

    tokio::fs::rename(part_path, final_zip_path).await?;
    Ok(outcome)
}

async fn try_download(
    http: &reqwest::Client,
    url: &str,
    part_path: &Path,
) -> Result<DownloadOutcome, DownloadError> {
    // Snapshot downloads are not retried at this layer: a partial transfer is
    // detected via the size check below and surfaced as a normal pipeline
    // failure for this version, to be retried wholesale on the *next* run
    // (design doc: "record status: failed ... retry next run"), rather than
    // silently resuming a possibly-inconsistent partial file.
    let mut response = http.get(url).send().await?.error_for_status()?;

    let content_length_header = response.content_length();
    let etag = header_str(&response, reqwest::header::ETAG);
    let last_modified = header_str(&response, reqwest::header::LAST_MODIFIED);

    let mut file = tokio::fs::File::create(part_path).await?;
    let mut hasher = Sha256::new();
    let mut total_bytes: u64 = 0;

    while let Some(chunk) = response.chunk().await? {
        hasher.update(&chunk);
        total_bytes += chunk.len() as u64;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;

    if let Some(expected) = content_length_header
        && expected != total_bytes
    {
        return Err(DownloadError::SizeMismatch {
            expected,
            actual: total_bytes,
        });
    }

    let sha256 = hex_encode(&hasher.finalize());

    Ok(DownloadOutcome {
        bytes: total_bytes,
        sha256,
        content_length_header,
        etag,
        last_modified,
    })
}

fn header_str(response: &reqwest::Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
