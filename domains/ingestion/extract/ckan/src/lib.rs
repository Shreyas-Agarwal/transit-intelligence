//! GTFS-S auto-downloader library. Implements
//! `docs/design/gtfs-static-auto-downloader.md` §§1–7.
//!
//! Split into a library + thin binary (`main.rs`) so integration tests can
//! exercise the pipeline, locking, and recovery logic directly.

pub mod archive;
pub mod ckan_client;
pub mod config;
pub mod domain;
pub mod download;
pub mod lock;
pub mod manifest;
pub mod paths;
pub mod pipeline;
pub mod symlink;
