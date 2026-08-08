//! Compiles `proto/gtfs-realtime.proto` (the canonical GTFS-RT spec, proto2
//! syntax) into Rust types via `prost-build`.
//!
//! The vendored `protoc` binary from `protoc-bin-vendored` is used explicitly
//! so the build does not depend on a system-installed `protoc`.

fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc binary");

    prost_build::Config::new()
        .protoc_executable(protoc)
        .compile_protos(&["proto/gtfs-realtime.proto"], &["proto"])
        .expect("compile gtfs-realtime.proto");

    println!("cargo:rerun-if-changed=proto/gtfs-realtime.proto");
}
