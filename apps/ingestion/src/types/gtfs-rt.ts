/**
 * TypeScript types representing decoded GTFS-RT entities.
 *
 * These mirror the protobuf message structure from gtfs-realtime.proto.
 * They describe the canonical JSON shape that the decoder produces and
 * the producer publishes to Redpanda.
 *
 * Naming follows the protobuf field names (snake_case preserved) so that
 * consumers can directly map back to the spec without translation.
 */

// ---------------------------------------------------------------------------
// Feed-level types
// ---------------------------------------------------------------------------

export interface FeedHeader {
  gtfs_realtime_version: string;
  incrementality: 'FULL_DATASET' | 'DIFFERENTIAL';
  timestamp: number; // POSIX seconds
}

export interface RawFeedMessage {
  header: FeedHeader;
  entity: FeedEntity[];
}

// ---------------------------------------------------------------------------
// Entity types
// ---------------------------------------------------------------------------

export type EntityType = 'VEHICLE_POSITION' | 'TRIP_UPDATE' | 'ALERT' | 'UNKNOWN';

export interface FeedEntity {
  id: string;
  is_deleted: boolean;
  vehicle?: VehiclePosition;
  trip_update?: TripUpdate;
  alert?: Alert;
}

// ---------------------------------------------------------------------------
// VehiclePosition
// ---------------------------------------------------------------------------

export interface Position {
  latitude: number;
  longitude: number;
  bearing?: number;
  odometer?: number;
  speed?: number; // m/s
}

export type VehicleStopStatus = 'INCOMING_AT' | 'STOPPED_AT' | 'IN_TRANSIT_TO';
export type CongestionLevel =
  | 'UNKNOWN_CONGESTION_LEVEL'
  | 'RUNNING_SMOOTHLY'
  | 'STOP_AND_GO'
  | 'CONGESTION'
  | 'SEVERE_CONGESTION';

export interface VehicleDescriptor {
  id?: string;
  label?: string;
  license_plate?: string;
}

export interface TripDescriptor {
  trip_id?: string;
  route_id?: string;
  direction_id?: number;
  start_time?: string; // HH:MM:SS
  start_date?: string; // YYYYMMDD
  schedule_relationship?: 'SCHEDULED' | 'ADDED' | 'UNSCHEDULED' | 'CANCELED' | 'REPLACEMENT';
}

export interface VehiclePosition {
  trip?: TripDescriptor;
  vehicle?: VehicleDescriptor;
  position?: Position;
  current_stop_sequence?: number;
  stop_id?: string;
  current_status?: VehicleStopStatus;
  timestamp?: number; // POSIX seconds
  congestion_level?: CongestionLevel;
}

// ---------------------------------------------------------------------------
// TripUpdate
// ---------------------------------------------------------------------------

export interface StopTimeEvent {
  delay?: number; // seconds (positive = late, negative = early)
  time?: number; // POSIX seconds (absolute)
  uncertainty?: number;
}

export type StopTimeUpdateScheduleRelationship = 'SCHEDULED' | 'SKIPPED' | 'NO_DATA';

export interface StopTimeUpdate {
  stop_sequence?: number;
  stop_id?: string;
  arrival?: StopTimeEvent;
  departure?: StopTimeEvent;
  schedule_relationship?: StopTimeUpdateScheduleRelationship;
}

export interface TripUpdate {
  trip: TripDescriptor;
  vehicle?: VehicleDescriptor;
  stop_time_update: StopTimeUpdate[];
  timestamp?: number; // POSIX seconds
  delay?: number; // seconds
}

// ---------------------------------------------------------------------------
// Alert
// ---------------------------------------------------------------------------

export interface TimeRange {
  start?: number; // POSIX seconds
  end?: number; // POSIX seconds
}

export interface EntitySelector {
  agency_id?: string;
  route_id?: string;
  route_type?: number;
  trip?: TripDescriptor;
  stop_id?: string;
}

export interface Translation {
  text: string;
  language?: string;
}

export interface TranslatedString {
  translation: Translation[];
}

export type AlertCause =
  | 'UNKNOWN_CAUSE'
  | 'OTHER_CAUSE'
  | 'TECHNICAL_PROBLEM'
  | 'STRIKE'
  | 'DEMONSTRATION'
  | 'ACCIDENT'
  | 'HOLIDAY'
  | 'WEATHER'
  | 'MAINTENANCE'
  | 'CONSTRUCTION'
  | 'POLICE_ACTIVITY'
  | 'MEDICAL_EMERGENCY';

export type AlertEffect =
  | 'NO_SERVICE'
  | 'REDUCED_SERVICE'
  | 'SIGNIFICANT_DELAYS'
  | 'DETOUR'
  | 'ADDITIONAL_SERVICE'
  | 'MODIFIED_SERVICE'
  | 'OTHER_EFFECT'
  | 'UNKNOWN_EFFECT'
  | 'STOP_MOVED';

export interface Alert {
  active_period: TimeRange[];
  informed_entity: EntitySelector[];
  cause?: AlertCause;
  effect?: AlertEffect;
  url?: TranslatedString;
  header_text?: TranslatedString;
  description_text?: TranslatedString;
}

// ---------------------------------------------------------------------------
// Canonical Redpanda message shape
// ---------------------------------------------------------------------------

/**
 * The envelope published to transit.snapshots.raw.
 * This is what `rpk topic consume transit.snapshots.raw` will display.
 */
export interface SnapshotRawMessage {
  /** Entity type discriminator */
  entity_type: EntityType;
  /** GTFS-RT entity id from the feed */
  entity_id: string;
  /** Feed header timestamp in ISO 8601 */
  feed_timestamp: string;
  /** Wall-clock time when this message was ingested, in ISO 8601 */
  ingestion_timestamp: string;
  /** GTFS-RT spec version from the feed header */
  feed_version: string;
  /** The decoded entity payload — one of vehicle, trip_update, or alert */
  payload: VehiclePosition | TripUpdate | Alert;
}
