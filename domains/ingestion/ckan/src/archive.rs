//! Tier 1 (archive-level) validation and extraction — design doc §5 steps 3–4,
//! §6. Deliberately does **not** parse any CSV content; that's Tier 2, owned by
//! the downstream DuckDB/SQLMesh layer per the design's language boundary.

use std::path::Path;

/// GTFS member files that must be present, non-empty, and intact for a snapshot
/// to be considered archive-sound (design doc §6, Tier 1).
const REQUIRED_MEMBERS: &[&str] = &[
    "stops.txt",
    "trips.txt",
    "routes.txt",
    "stop_times.txt",
    "calendar_dates.txt",
];

/// Present in most feeds but not required by this design's Tier 1 check; if
/// present, they get the same non-zero-size scrutiny as required members.
const OPTIONAL_MEMBERS: &[&str] = &["agency.txt", "calendar.txt"];

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a valid zip archive: {0}")]
    InvalidZip(String),
    #[error("archive entry {0:?} failed its CRC32 check (corrupt member)")]
    CrcMismatch(String),
    #[error("archive is missing required GTFS member file {0:?}")]
    MissingRequiredMember(&'static str),
    #[error("archive member {0:?} has zero size")]
    EmptyMember(String),
    #[error("extracted file {0:?} is missing on disk after extraction")]
    MissingAfterExtraction(&'static str),
    #[error("extracted file {0:?} has zero size on disk after extraction")]
    EmptyAfterExtraction(&'static str),
}

/// Validates the zip at `zip_path` structurally, then extracts it into
/// `extract_dir` (which must already exist and be empty — staging directories
/// are created fresh per snapshot). Returns an error at the first problem found
/// without partially extracting a known-bad archive.
pub fn validate_and_extract(zip_path: &Path, extract_dir: &Path) -> Result<(), ArchiveError> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| ArchiveError::InvalidZip(e.to_string()))?;

    validate_members(&mut archive)?;

    archive
        .extract(extract_dir)
        .map_err(|e| ArchiveError::InvalidZip(e.to_string()))?;

    verify_extracted_members(extract_dir)?;

    Ok(())
}

/// Pass 1: for every entry, force a full decompress into a sink to trigger the
/// zip crate's CRC32 check (it validates on read-to-EOF), and record the
/// uncompressed size of any required/optional GTFS member by name.
fn validate_members(archive: &mut zip::ZipArchive<std::fs::File>) -> Result<(), ArchiveError> {
    let mut member_sizes: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ArchiveError::InvalidZip(e.to_string()))?;
        let name = entry.name().to_string();
        let size = entry.size();

        let mut sink = std::io::sink();
        std::io::copy(&mut entry, &mut sink)
            .map_err(|_| ArchiveError::CrcMismatch(name.clone()))?;

        if let Some(base_name) = member_base_name(&name) {
            member_sizes.insert(base_name, size);
        }
    }

    for required in REQUIRED_MEMBERS {
        match member_sizes.get(*required) {
            None => return Err(ArchiveError::MissingRequiredMember(required)),
            Some(0) => return Err(ArchiveError::EmptyMember((*required).to_string())),
            Some(_) => {}
        }
    }

    for optional in OPTIONAL_MEMBERS {
        if let Some(0) = member_sizes.get(*optional) {
            return Err(ArchiveError::EmptyMember((*optional).to_string()));
        }
    }

    Ok(())
}

/// After extraction, re-check required members directly on disk — catches
/// truncation introduced by the extraction/filesystem-write step itself, which
/// the in-memory CRC check above can't see (design doc §6: "catches truncated
/// members that a naive 'does the file exist' check would miss").
fn verify_extracted_members(extract_dir: &Path) -> Result<(), ArchiveError> {
    for required in REQUIRED_MEMBERS {
        let path = extract_dir.join(required);
        let metadata =
            std::fs::metadata(&path).map_err(|_| ArchiveError::MissingAfterExtraction(required))?;
        if metadata.len() == 0 {
            return Err(ArchiveError::EmptyAfterExtraction(required));
        }
    }
    Ok(())
}

/// GTFS member files are expected at the top level of the archive per this
/// design's Tier 1 scope; matches case-insensitively on the file's base name so
/// a differently-cased upstream zip (`Stops.txt`) still validates, but ignores
/// entries nested in subdirectories rather than guessing at a search strategy
/// the design doesn't specify.
fn member_base_name(entry_name: &str) -> Option<String> {
    let path = Path::new(entry_name);
    if path.parent().is_some_and(|p| p != Path::new("")) {
        return None;
    }
    path.file_name().map(|n| n.to_string_lossy().to_lowercase())
}
