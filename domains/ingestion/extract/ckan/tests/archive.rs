//! Design doc §6, Tier 1: archive-level structural validation only — a valid
//! zip with all required GTFS members passes; missing/empty members and
//! corrupt entries are rejected before anything is published.

use std::io::Write;

use ckan::archive::{ArchiveError, validate_and_extract};

const REQUIRED: &[&str] = &[
    "stops.txt",
    "trips.txt",
    "routes.txt",
    "stop_times.txt",
    "calendar_dates.txt",
];

fn build_zip(path: &std::path::Path, members: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for (name, contents) in members {
        writer.start_file(*name, options).unwrap();
        writer.write_all(contents).unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn valid_gtfs_zip_passes_and_extracts() {
    let tmp = tempfile::tempdir().unwrap();
    let zip_path = tmp.path().join("snapshot.zip");
    let members: Vec<(&str, &[u8])> = REQUIRED
        .iter()
        .map(|&n| (n, b"col_a,col_b\n1,2\n" as &[u8]))
        .collect();
    build_zip(&zip_path, &members);

    let extract_dir = tmp.path().join("extracted");
    std::fs::create_dir_all(&extract_dir).unwrap();

    validate_and_extract(&zip_path, &extract_dir).unwrap();

    for name in REQUIRED {
        let extracted = extract_dir.join(name);
        assert!(extracted.exists(), "{name} should exist after extraction");
        assert!(std::fs::metadata(&extracted).unwrap().len() > 0);
    }
}

#[test]
fn missing_required_member_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let zip_path = tmp.path().join("snapshot.zip");
    // Missing calendar_dates.txt.
    let members: [(&str, &[u8]); 4] = [
        ("stops.txt", b"a\n"),
        ("trips.txt", b"a\n"),
        ("routes.txt", b"a\n"),
        ("stop_times.txt", b"a\n"),
    ];
    build_zip(&zip_path, &members);

    let extract_dir = tmp.path().join("extracted");
    std::fs::create_dir_all(&extract_dir).unwrap();

    let result = validate_and_extract(&zip_path, &extract_dir);
    assert!(result.is_err());
}

#[test]
fn zero_size_required_member_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let zip_path = tmp.path().join("snapshot.zip");
    let mut members: Vec<(&str, &[u8])> = REQUIRED.iter().map(|&n| (n, b"a\n" as &[u8])).collect();
    // Overwrite stops.txt with an empty member.
    members[0] = ("stops.txt", b"");
    build_zip(&zip_path, &members);

    let extract_dir = tmp.path().join("extracted");
    std::fs::create_dir_all(&extract_dir).unwrap();

    let result = validate_and_extract(&zip_path, &extract_dir);
    assert!(result.is_err());
}

#[test]
fn not_a_zip_file_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let zip_path = tmp.path().join("snapshot.zip");
    std::fs::write(&zip_path, b"this is not a zip file").unwrap();

    let extract_dir = tmp.path().join("extracted");
    std::fs::create_dir_all(&extract_dir).unwrap();

    let result = validate_and_extract(&zip_path, &extract_dir);
    assert!(result.is_err());
}

/// A structurally well-formed zip (correct headers, correct central
/// directory, a required member present and non-empty) whose actual bytes
/// were corrupted after the CRC32 was recorded — the "silent bit rot" case,
/// distinct from a missing member or a file that isn't a zip at all — must
/// be rejected specifically as a CRC failure, not partially extracted.
///
/// Implementation plan Phase 9's "corrupt archive" failure mode had no
/// dedicated test before this, despite `ArchiveError::CrcMismatch` existing
/// specifically to handle it.
#[test]
fn a_member_corrupted_after_writing_fails_its_crc_check() {
    let tmp = tempfile::tempdir().unwrap();
    let zip_path = tmp.path().join("snapshot.zip");

    // Stored (uncompressed) so the member's real bytes appear verbatim in
    // the archive — flipping one of them changes the actual content without
    // touching any zip structure, which is exactly what corruption in
    // transit or on disk looks like: the container is intact, the payload
    // isn't.
    const MARKER: &[u8] = b"CORRUPT_ME_MARKER_0001";
    let mut buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for name in REQUIRED {
            writer.start_file(*name, stored).unwrap();
            if *name == "stops.txt" {
                writer.write_all(MARKER).unwrap();
            } else {
                writer.write_all(b"a\n").unwrap();
            }
        }
        writer.finish().unwrap();
    }
    let pos = buf
        .windows(MARKER.len())
        .position(|w| w == MARKER)
        .expect("the marker must appear verbatim in a Stored entry");
    buf[pos] ^= 0xFF;
    std::fs::write(&zip_path, &buf).unwrap();

    let extract_dir = tmp.path().join("extracted");
    std::fs::create_dir_all(&extract_dir).unwrap();

    let result = validate_and_extract(&zip_path, &extract_dir);
    assert!(
        matches!(result, Err(ArchiveError::CrcMismatch(_))),
        "expected a CRC mismatch, got {result:?}"
    );
    assert!(
        !extract_dir.join("stops.txt").exists(),
        "a known-bad archive must not be partially extracted"
    );
}
