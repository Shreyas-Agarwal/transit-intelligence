//! Design doc §8: Parquet is the canonical, permanently-persisted storage
//! format for extracted GTFS snapshots. Every `*.txt` present gets a
//! same-named `*.parquet` sibling; every column round-trips as a string
//! regardless of its apparent shape (see `parquet_convert`'s module doc for
//! why — this is bytes-format conversion, not GTFS-typed reasoning).

use std::fs::File;

use arrow_array::{Array, StringArray};
use ckan::parquet_convert::convert_directory;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

fn read_all_as_strings(path: &std::path::Path, column: &str) -> Vec<Option<String>> {
    let file = File::open(path).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap()
        .build()
        .unwrap();

    let mut values = Vec::new();
    for batch in reader {
        let batch = batch.unwrap();
        let idx = batch.schema().index_of(column).unwrap();
        let col = batch
            .column(idx)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        for i in 0..col.len() {
            values.push((!col.is_null(i)).then(|| col.value(i).to_string()));
        }
    }
    values
}

#[test]
fn converts_every_csv_member_to_a_same_named_parquet_file() {
    let tmp = tempfile::tempdir().unwrap();
    let csv_dir = tmp.path().join("csv");
    let parquet_dir = tmp.path().join("parquet");
    std::fs::create_dir_all(&csv_dir).unwrap();
    std::fs::create_dir_all(&parquet_dir).unwrap();

    std::fs::write(
        csv_dir.join("stops.txt"),
        "stop_id,stop_name,stop_lat\nS1,Zurich HB,47.378\nS2,Bern,46.949\n",
    )
    .unwrap();
    std::fs::write(csv_dir.join("routes.txt"), "route_id\nR1\nR2\n").unwrap();
    // Not a GTFS CSV — should be ignored, not converted.
    std::fs::write(csv_dir.join("README.md"), "not a csv").unwrap();

    convert_directory(&csv_dir, &parquet_dir).unwrap();

    assert!(parquet_dir.join("stops.parquet").exists());
    assert!(parquet_dir.join("routes.parquet").exists());
    assert!(!parquet_dir.join("README.parquet").exists());

    let stop_ids = read_all_as_strings(&parquet_dir.join("stops.parquet"), "stop_id");
    assert_eq!(
        stop_ids,
        vec![Some("S1".to_string()), Some("S2".to_string())]
    );

    // Numeric-looking column still round-trips as a string, unrounded and
    // unreformatted — the conversion doesn't interpret GTFS column semantics.
    let lats = read_all_as_strings(&parquet_dir.join("stops.parquet"), "stop_lat");
    assert_eq!(
        lats,
        vec![Some("47.378".to_string()), Some("46.949".to_string())]
    );
}

#[test]
fn preserves_row_count_for_a_larger_file() {
    let tmp = tempfile::tempdir().unwrap();
    let csv_dir = tmp.path().join("csv");
    let parquet_dir = tmp.path().join("parquet");
    std::fs::create_dir_all(&csv_dir).unwrap();
    std::fs::create_dir_all(&parquet_dir).unwrap();

    let mut csv = String::from("trip_id,route_id\n");
    for i in 0..5000 {
        csv.push_str(&format!("T{i},R{}\n", i % 10));
    }
    std::fs::write(csv_dir.join("trips.txt"), csv).unwrap();

    convert_directory(&csv_dir, &parquet_dir).unwrap();

    let trip_ids = read_all_as_strings(&parquet_dir.join("trips.parquet"), "trip_id");
    assert_eq!(trip_ids.len(), 5000);
    assert_eq!(trip_ids[0], Some("T0".to_string()));
    assert_eq!(trip_ids[4999], Some("T4999".to_string()));
}
