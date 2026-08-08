//! Design doc §6, Tier 1: archive-level structural validation only — a valid
//! zip with all required GTFS members passes; missing/empty members and
//! corrupt entries are rejected before anything is published.

use std::io::Write;

use ckan::archive::validate_and_extract;

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
