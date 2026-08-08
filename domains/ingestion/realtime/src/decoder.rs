//! Decodes a binary GTFS-RT protobuf buffer into the generated
//! [`proto::FeedMessage`] type.

use anyhow::{Context, Result};
use prost::Message;

use crate::proto;

pub fn decode_feed_buffer(buffer: &[u8]) -> Result<proto::FeedMessage> {
    proto::FeedMessage::decode(buffer).context("failed to decode GTFS-RT FeedMessage")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(msg: &proto::FeedMessage) -> Vec<u8> {
        let mut buf = Vec::new();
        msg.encode(&mut buf).unwrap();
        buf
    }

    #[test]
    fn round_trips_the_feed_header() {
        let msg = proto::FeedMessage {
            header: proto::FeedHeader {
                gtfs_realtime_version: "2.0".to_string(),
                incrementality: Some(proto::feed_header::Incrementality::FullDataset as i32),
                timestamp: Some(1_749_722_400),
            },
            entity: vec![],
        };

        let decoded = decode_feed_buffer(&encode(&msg)).unwrap();
        assert_eq!(decoded.header.gtfs_realtime_version, "2.0");
        assert_eq!(decoded.header.timestamp, Some(1_749_722_400));
        assert_eq!(
            proto::feed_header::Incrementality::try_from(decoded.header.incrementality.unwrap())
                .unwrap(),
            proto::feed_header::Incrementality::FullDataset,
        );
    }

    #[test]
    fn round_trips_a_vehicle_position_entity() {
        let msg = proto::FeedMessage {
            header: proto::FeedHeader {
                gtfs_realtime_version: "2.0".to_string(),
                incrementality: None,
                timestamp: None,
            },
            entity: vec![proto::FeedEntity {
                id: "vehicle-entity-001".to_string(),
                is_deleted: None,
                trip_update: None,
                vehicle: Some(proto::VehiclePosition {
                    trip: None,
                    vehicle: Some(proto::VehicleDescriptor {
                        id: Some("ch:vbz:tram:3001".to_string()),
                        label: None,
                        license_plate: None,
                    }),
                    position: Some(proto::Position {
                        latitude: 47.3769,
                        longitude: 8.5417,
                        bearing: Some(270.0),
                        odometer: None,
                        speed: Some(8.3),
                    }),
                    current_stop_sequence: None,
                    stop_id: None,
                    current_status: None,
                    timestamp: None,
                    congestion_level: None,
                    occupancy_status: None,
                }),
                alert: None,
                shape: None,
            }],
        };

        let decoded = decode_feed_buffer(&encode(&msg)).unwrap();
        assert_eq!(decoded.entity.len(), 1);
        let vehicle = decoded.entity[0].vehicle.as_ref().unwrap();
        assert_eq!(
            vehicle.vehicle.as_ref().unwrap().id.as_deref(),
            Some("ch:vbz:tram:3001")
        );
        assert!((vehicle.position.as_ref().unwrap().latitude - 47.3769).abs() < f32::EPSILON);
    }

    #[test]
    fn rejects_garbage_bytes() {
        assert!(decode_feed_buffer(&[0xff, 0x00, 0x01]).is_err());
    }
}
