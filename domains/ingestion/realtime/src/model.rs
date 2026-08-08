//! JSON-serializable GTFS-RT model, converted from the generated protobuf
//! types (`proto`), plus the pure functions that turn decoded entities into
//! the `transit.snapshots.raw` envelope (see
//! `docs/design/redpanda-topic-configuration.md`).
//!
//! Enum fields are converted to their proto string name (`as_str_name()`)
//! and, mirroring the original decoder's `defaults: false` behaviour, are
//! only present in the output when the field was actually set on the wire —
//! a field's `[default = ...]` is not backfilled here.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::proto;

fn enum_name<T>(raw: Option<i32>) -> Option<&'static str>
where
    T: TryFrom<i32> + EnumName,
{
    raw.and_then(|v| T::try_from(v).ok()).map(|v| v.name())
}

trait EnumName {
    fn name(&self) -> &'static str;
}

macro_rules! impl_enum_name {
    ($ty:ty) => {
        impl EnumName for $ty {
            fn name(&self) -> &'static str {
                self.as_str_name()
            }
        }
    };
}

impl_enum_name!(proto::feed_header::Incrementality);
impl_enum_name!(proto::vehicle_position::VehicleStopStatus);
impl_enum_name!(proto::vehicle_position::CongestionLevel);
impl_enum_name!(proto::vehicle_position::OccupancyStatus);
impl_enum_name!(proto::trip_descriptor::ScheduleRelationship);
impl_enum_name!(proto::trip_update::stop_time_update::ScheduleRelationship);
impl_enum_name!(proto::alert::Cause);
impl_enum_name!(proto::alert::Effect);

// ---------------------------------------------------------------------------
// Feed-level types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct FeedHeader {
    pub gtfs_realtime_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incrementality: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

impl From<&proto::FeedHeader> for FeedHeader {
    fn from(h: &proto::FeedHeader) -> Self {
        Self {
            gtfs_realtime_version: h.gtfs_realtime_version.clone(),
            incrementality: enum_name::<proto::feed_header::Incrementality>(h.incrementality),
            timestamp: h.timestamp,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared descriptor types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub latitude: f32,
    pub longitude: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearing: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub odometer: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
}

impl From<&proto::Position> for Position {
    fn from(p: &proto::Position) -> Self {
        Self {
            latitude: p.latitude,
            longitude: p.longitude,
            bearing: p.bearing,
            odometer: p.odometer,
            speed: p.speed,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VehicleDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_plate: Option<String>,
}

impl From<&proto::VehicleDescriptor> for VehicleDescriptor {
    fn from(v: &proto::VehicleDescriptor) -> Self {
        Self {
            id: v.id.clone(),
            label: v.label.clone(),
            license_plate: v.license_plate.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TripDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trip_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_relationship: Option<&'static str>,
}

impl From<&proto::TripDescriptor> for TripDescriptor {
    fn from(t: &proto::TripDescriptor) -> Self {
        Self {
            trip_id: t.trip_id.clone(),
            route_id: t.route_id.clone(),
            direction_id: t.direction_id,
            start_time: t.start_time.clone(),
            start_date: t.start_date.clone(),
            schedule_relationship: enum_name::<proto::trip_descriptor::ScheduleRelationship>(
                t.schedule_relationship,
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// VehiclePosition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct VehiclePosition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trip: Option<TripDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<VehicleDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_stop_sequence: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_status: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub congestion_level: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occupancy_status: Option<&'static str>,
}

impl From<&proto::VehiclePosition> for VehiclePosition {
    fn from(v: &proto::VehiclePosition) -> Self {
        Self {
            trip: v.trip.as_ref().map(Into::into),
            vehicle: v.vehicle.as_ref().map(Into::into),
            position: v.position.as_ref().map(Into::into),
            current_stop_sequence: v.current_stop_sequence,
            stop_id: v.stop_id.clone(),
            current_status: enum_name::<proto::vehicle_position::VehicleStopStatus>(
                v.current_status,
            ),
            timestamp: v.timestamp,
            congestion_level: enum_name::<proto::vehicle_position::CongestionLevel>(
                v.congestion_level,
            ),
            occupancy_status: enum_name::<proto::vehicle_position::OccupancyStatus>(
                v.occupancy_status,
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// TripUpdate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct StopTimeEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<i32>,
}

impl From<&proto::trip_update::StopTimeEvent> for StopTimeEvent {
    fn from(e: &proto::trip_update::StopTimeEvent) -> Self {
        Self {
            delay: e.delay,
            time: e.time,
            uncertainty: e.uncertainty,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StopTimeUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrival: Option<StopTimeEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departure: Option<StopTimeEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_relationship: Option<&'static str>,
}

impl From<&proto::trip_update::StopTimeUpdate> for StopTimeUpdate {
    fn from(u: &proto::trip_update::StopTimeUpdate) -> Self {
        Self {
            stop_sequence: u.stop_sequence,
            stop_id: u.stop_id.clone(),
            arrival: u.arrival.as_ref().map(Into::into),
            departure: u.departure.as_ref().map(Into::into),
            schedule_relationship: enum_name::<
                proto::trip_update::stop_time_update::ScheduleRelationship,
            >(u.schedule_relationship),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TripUpdate {
    pub trip: TripDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<VehicleDescriptor>,
    pub stop_time_update: Vec<StopTimeUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<i32>,
}

impl From<&proto::TripUpdate> for TripUpdate {
    fn from(t: &proto::TripUpdate) -> Self {
        Self {
            trip: (&t.trip).into(),
            vehicle: t.vehicle.as_ref().map(Into::into),
            stop_time_update: t.stop_time_update.iter().map(Into::into).collect(),
            timestamp: t.timestamp,
            delay: t.delay,
        }
    }
}

// ---------------------------------------------------------------------------
// Alert
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct TimeRange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<u64>,
}

impl From<&proto::TimeRange> for TimeRange {
    fn from(t: &proto::TimeRange) -> Self {
        Self {
            start: t.start,
            end: t.end,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EntitySelector {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agency_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_type: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trip: Option<TripDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_id: Option<String>,
}

impl From<&proto::EntitySelector> for EntitySelector {
    fn from(e: &proto::EntitySelector) -> Self {
        Self {
            agency_id: e.agency_id.clone(),
            route_id: e.route_id.clone(),
            route_type: e.route_type,
            trip: e.trip.as_ref().map(Into::into),
            stop_id: e.stop_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Translation {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

impl From<&proto::translated_string::Translation> for Translation {
    fn from(t: &proto::translated_string::Translation) -> Self {
        Self {
            text: t.text.clone(),
            language: t.language.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TranslatedString {
    pub translation: Vec<Translation>,
}

impl From<&proto::TranslatedString> for TranslatedString {
    fn from(t: &proto::TranslatedString) -> Self {
        Self {
            translation: t.translation.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Alert {
    pub active_period: Vec<TimeRange>,
    pub informed_entity: Vec<EntitySelector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<TranslatedString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_text: Option<TranslatedString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<TranslatedString>,
}

impl From<&proto::Alert> for Alert {
    fn from(a: &proto::Alert) -> Self {
        Self {
            active_period: a.active_period.iter().map(Into::into).collect(),
            informed_entity: a.informed_entity.iter().map(Into::into).collect(),
            cause: enum_name::<proto::alert::Cause>(a.cause),
            effect: enum_name::<proto::alert::Effect>(a.effect),
            url: a.url.as_ref().map(Into::into),
            header_text: a.header_text.as_ref().map(Into::into),
            description_text: a.description_text.as_ref().map(Into::into),
        }
    }
}

// ---------------------------------------------------------------------------
// Entity type discriminator + envelope
// ---------------------------------------------------------------------------

pub type EntityType = &'static str;

/// Returns the entity type discriminator string for a `FeedEntity`.
///
/// Priority order matches the GTFS-RT proto field ordering:
/// `trip_update` → `vehicle` → `alert`. A `FeedEntity` carries at most one
/// payload field, so the order only matters for the `UNKNOWN` fallback.
pub fn entity_type(entity: &proto::FeedEntity) -> EntityType {
    if entity.vehicle.is_some() {
        "VEHICLE_POSITION"
    } else if entity.trip_update.is_some() {
        "TRIP_UPDATE"
    } else if entity.alert.is_some() {
        "ALERT"
    } else {
        "UNKNOWN"
    }
}

/// Derives a stable Redpanda partition key for a given GTFS-RT entity.
///
/// See `docs/design/gtfs-rt-domain-mapping.md` for the key strategy:
/// vehicle positions and trip updates key off their natural id (falling back
/// to `entity.id` when the descriptor is absent); alerts — whose ids aren't
/// guaranteed stable across providers — key off a short sha256 prefix of
/// `entity.id` instead.
pub fn derive_key(entity: &proto::FeedEntity) -> String {
    if let Some(v) = &entity.vehicle {
        let vehicle_id = v
            .vehicle
            .as_ref()
            .and_then(|d| d.id.clone())
            .unwrap_or_else(|| entity.id.clone());
        format!("vehicle.{vehicle_id}")
    } else if let Some(t) = &entity.trip_update {
        let trip_id = t.trip.trip_id.clone().unwrap_or_else(|| entity.id.clone());
        format!("trip.{trip_id}")
    } else if entity.alert.is_some() {
        let digest = Sha256::digest(entity.id.as_bytes());
        let hex = digest[..6]
            .iter()
            .fold(String::with_capacity(12), |mut acc, b| {
                use std::fmt::Write;
                write!(acc, "{b:02x}").expect("String write is infallible");
                acc
            });
        format!("alert.{hex}")
    } else {
        format!("unknown.{}", entity.id)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Payload {
    Vehicle(VehiclePosition),
    TripUpdate(TripUpdate),
    Alert(Alert),
    Empty {},
}

/// The envelope published to `transit.snapshots.raw`.
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotRawMessage {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub feed_timestamp: String,
    pub ingestion_timestamp: String,
    pub feed_version: String,
    pub payload: Payload,
}

pub struct KafkaMessage {
    pub key: String,
    /// JSON string — human-readable via `rpk topic consume`.
    pub value: String,
}

/// Wraps a list of `FeedEntity` records in `SnapshotRawMessage` envelopes and
/// serialises each to a JSON string ready for Redpanda.
///
/// `is_deleted` entities are silently dropped — they appear only in
/// DIFFERENTIAL mode feeds; the Swiss feed uses FULL_DATASET.
/// `ingestion_timestamp` is passed in rather than sampled per-entity, so all
/// messages from the same poll cycle share the same processing timestamp.
pub fn build_messages(
    entities: &[proto::FeedEntity],
    feed_timestamp_iso: &str,
    feed_version: &str,
    ingestion_timestamp: &str,
) -> Vec<KafkaMessage> {
    entities
        .iter()
        .filter(|e| !e.is_deleted.unwrap_or(false))
        .map(|entity| {
            let payload = if let Some(v) = &entity.vehicle {
                Payload::Vehicle(v.into())
            } else if let Some(t) = &entity.trip_update {
                Payload::TripUpdate(t.into())
            } else if let Some(a) = &entity.alert {
                Payload::Alert(a.into())
            } else {
                Payload::Empty {}
            };

            let envelope = SnapshotRawMessage {
                entity_type: entity_type(entity),
                entity_id: entity.id.clone(),
                feed_timestamp: feed_timestamp_iso.to_string(),
                ingestion_timestamp: ingestion_timestamp.to_string(),
                feed_version: feed_version.to_string(),
                payload,
            };

            KafkaMessage {
                key: derive_key(entity),
                value: serde_json::to_string(&envelope)
                    .expect("SnapshotRawMessage serializes to JSON"),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vehicle_entity(id: &str, vehicle_id: Option<&str>) -> proto::FeedEntity {
        proto::FeedEntity {
            id: id.to_string(),
            is_deleted: None,
            trip_update: None,
            vehicle: Some(proto::VehiclePosition {
                trip: None,
                vehicle: vehicle_id.map(|v| proto::VehicleDescriptor {
                    id: Some(v.to_string()),
                    label: None,
                    license_plate: None,
                }),
                position: None,
                current_stop_sequence: None,
                stop_id: None,
                current_status: None,
                timestamp: None,
                congestion_level: None,
                occupancy_status: None,
            }),
            alert: None,
            shape: None,
        }
    }

    fn trip_update_entity(id: &str, trip_id: Option<&str>) -> proto::FeedEntity {
        proto::FeedEntity {
            id: id.to_string(),
            is_deleted: None,
            trip_update: Some(proto::TripUpdate {
                trip: proto::TripDescriptor {
                    trip_id: trip_id.map(str::to_string),
                    route_id: None,
                    direction_id: None,
                    start_time: None,
                    start_date: None,
                    schedule_relationship: None,
                },
                vehicle: None,
                stop_time_update: vec![],
                timestamp: None,
                delay: None,
            }),
            vehicle: None,
            alert: None,
            shape: None,
        }
    }

    fn alert_entity(id: &str) -> proto::FeedEntity {
        proto::FeedEntity {
            id: id.to_string(),
            is_deleted: None,
            trip_update: None,
            vehicle: None,
            alert: Some(proto::Alert {
                active_period: vec![],
                informed_entity: vec![],
                cause: None,
                effect: None,
                url: None,
                header_text: None,
                description_text: None,
            }),
            shape: None,
        }
    }

    #[test]
    fn entity_type_prioritises_vehicle_over_trip_and_alert() {
        assert_eq!(entity_type(&vehicle_entity("e1", None)), "VEHICLE_POSITION");
        assert_eq!(entity_type(&trip_update_entity("e2", None)), "TRIP_UPDATE");
        assert_eq!(entity_type(&alert_entity("e3")), "ALERT");
    }

    #[test]
    fn entity_type_falls_back_to_unknown() {
        let entity = proto::FeedEntity {
            id: "e4".to_string(),
            is_deleted: None,
            trip_update: None,
            vehicle: None,
            alert: None,
            shape: None,
        };
        assert_eq!(entity_type(&entity), "UNKNOWN");
    }

    #[test]
    fn derive_key_uses_vehicle_id_when_present() {
        let entity = vehicle_entity("e1", Some("ch:vbz:tram:3001"));
        assert_eq!(derive_key(&entity), "vehicle.ch:vbz:tram:3001");
    }

    #[test]
    fn derive_key_falls_back_to_entity_id_for_vehicle() {
        let entity = vehicle_entity("e1", None);
        assert_eq!(derive_key(&entity), "vehicle.e1");
    }

    #[test]
    fn derive_key_uses_trip_id_when_present() {
        let entity = trip_update_entity("e2", Some("trip:sbb:8001"));
        assert_eq!(derive_key(&entity), "trip.trip:sbb:8001");
    }

    #[test]
    fn derive_key_hashes_entity_id_for_alerts() {
        let entity = alert_entity("alert-entity-001");
        let key = derive_key(&entity);
        assert!(key.starts_with("alert."));
        assert_eq!(key.len(), "alert.".len() + 12);
        // deterministic — same input always yields the same key.
        assert_eq!(key, derive_key(&alert_entity("alert-entity-001")));
    }

    #[test]
    fn derive_key_falls_back_to_unknown_prefix() {
        let entity = proto::FeedEntity {
            id: "e5".to_string(),
            is_deleted: None,
            trip_update: None,
            vehicle: None,
            alert: None,
            shape: None,
        };
        assert_eq!(derive_key(&entity), "unknown.e5");
    }

    #[test]
    fn build_messages_drops_deleted_entities() {
        let mut entity = vehicle_entity("e1", Some("v1"));
        entity.is_deleted = Some(true);
        let messages = build_messages(
            &[entity],
            "2026-01-01T00:00:00Z",
            "2.0",
            "2026-01-01T00:00:01Z",
        );
        assert!(messages.is_empty());
    }

    #[test]
    fn build_messages_wraps_entities_in_envelope() {
        let entity = trip_update_entity("e2", Some("trip:sbb:8001"));
        let messages = build_messages(
            &[entity],
            "2026-01-01T00:00:00Z",
            "2.0",
            "2026-01-01T00:00:01Z",
        );
        assert_eq!(messages.len(), 1);
        let value: serde_json::Value = serde_json::from_str(&messages[0].value).unwrap();
        assert_eq!(value["entity_type"], "TRIP_UPDATE");
        assert_eq!(value["entity_id"], "e2");
        assert_eq!(value["feed_version"], "2.0");
        assert_eq!(value["payload"]["trip"]["trip_id"], "trip:sbb:8001");
    }
}
