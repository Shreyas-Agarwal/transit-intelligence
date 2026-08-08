//! CSV -> Parquet conversion, run once per snapshot immediately after Tier 1
//! (archive-level) validation passes (design doc §5, §6, §8).
//!
//! This stays firmly on the Rust side of the [language & tooling
//! boundary](../../../docs/design/gtfs-static-auto-downloader.md): it's a
//! bytes-format conversion, not GTFS-typed reasoning. Every column is read and
//! written as `Utf8` — no attempt is made to infer that `stop_lat` is a float
//! or `stop_sequence` is an integer. That reasoning belongs to whatever reads
//! the Parquet back (the DuckDB/SQLMesh layer, which already casts explicit
//! types per column). Treating everything as Utf8 also sidesteps GTFS's usual
//! CSV type hazards (times past `24:00:00`, blank-vs-null optional fields,
//! leading zeros in IDs) that a naive type-inferring converter would trip over.
//!
//! Parquet's own dictionary + run-length encoding still compresses
//! highly-repetitive transit data (stop IDs, route IDs, service IDs) very
//! effectively even with every column typed as a string.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_csv::ReaderBuilder;
use arrow_csv::reader::Format;
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

#[derive(Debug, thiserror::Error)]
pub enum ParquetError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to read CSV {0:?}: {1}")]
    CsvRead(PathBuf, arrow_schema::ArrowError),
    #[error("failed to write parquet {0:?}: {1}")]
    ParquetWrite(PathBuf, parquet::errors::ParquetError),
}

/// Converts every `*.txt` CSV member in `csv_dir` into a same-named `*.parquet`
/// file in `parquet_dir` (which must already exist). Converts whatever CSV
/// files are actually present — required or optional GTFS members alike — so
/// this doesn't need updating every time the GTFS spec grows an optional file.
pub fn convert_directory(csv_dir: &Path, parquet_dir: &Path) -> Result<(), ParquetError> {
    for entry in std::fs::read_dir(csv_dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_csv = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"));
        if !entry.file_type()?.is_file() || !is_csv {
            continue;
        }

        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let parquet_path = parquet_dir.join(format!("{stem}.parquet"));
        convert_file(&path, &parquet_path)?;
    }
    Ok(())
}

/// Converts one CSV file to one Parquet file, both fully in memory as a single
/// batch — acceptable here since a snapshot's largest member
/// (`stop_times.txt`, tens of millions of rows, per ADR 0011) is still on the
/// order of a few hundred MB as UTF-8 text, and this runs once per snapshot on
/// a twice-weekly cadence, not on a hot path.
fn convert_file(csv_path: &Path, parquet_path: &Path) -> Result<(), ParquetError> {
    let header_file = File::open(csv_path)
        .map_err(|e| ParquetError::CsvRead(csv_path.to_path_buf(), e.into()))?;
    let (header_schema, _) = Format::default()
        .with_header(true)
        .infer_schema(header_file, Some(0))
        .map_err(|e| ParquetError::CsvRead(csv_path.to_path_buf(), e))?;

    // Force every column to Utf8 regardless of what the (unused) inferred
    // types above came out as — see the module doc comment.
    let schema = Arc::new(Schema::new(
        header_schema
            .fields()
            .iter()
            .map(|f| Field::new(f.name(), DataType::Utf8, true))
            .collect::<Vec<_>>(),
    ));

    let data_file = File::open(csv_path)
        .map_err(|e| ParquetError::CsvRead(csv_path.to_path_buf(), e.into()))?;
    let mut csv_reader = ReaderBuilder::new(schema.clone())
        .with_header(true)
        .build(data_file)
        .map_err(|e| ParquetError::CsvRead(csv_path.to_path_buf(), e))?;

    let output = File::create(parquet_path)
        .map_err(|e| ParquetError::CsvRead(parquet_path.to_path_buf(), e.into()))?;
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .build();
    let mut writer = ArrowWriter::try_new(output, schema, Some(props))
        .map_err(|e| ParquetError::ParquetWrite(parquet_path.to_path_buf(), e))?;

    for batch in &mut csv_reader {
        let batch = batch.map_err(|e| ParquetError::CsvRead(csv_path.to_path_buf(), e))?;
        writer
            .write(&batch)
            .map_err(|e| ParquetError::ParquetWrite(parquet_path.to_path_buf(), e))?;
    }

    writer
        .close()
        .map_err(|e| ParquetError::ParquetWrite(parquet_path.to_path_buf(), e))?;

    Ok(())
}
