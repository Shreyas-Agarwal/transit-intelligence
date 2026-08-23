//! The OpenTelemetry resource attributes every ingestion binary reports
//! identically — which binary and which build produced a given span or
//! metric — so that's answered the same way everywhere instead of each
//! crate inventing its own labels.

use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;

use super::ServiceInfo;

pub(super) fn build(service: &ServiceInfo) -> Resource {
    Resource::builder()
        .with_service_name(service.name)
        .with_attribute(KeyValue::new("service.version", service.version))
        .build()
}
