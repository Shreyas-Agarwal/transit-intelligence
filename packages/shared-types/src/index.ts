export interface Coordinate {
  latitude: number;
  longitude: number;
}

export interface TelemetryPayload {
  vehicleId: string;
  timestamp: string;
  coordinate: Coordinate;
  speed: number; // Speed in km/h
  heading: number; // Heading in degrees (0-359)
  odometer?: number;
}

export type VehicleStatus = 'ACTIVE' | 'INACTIVE' | 'MAINTENANCE';

export interface Vehicle {
  id: string;
  licensePlate: string;
  status: VehicleStatus;
  capacity: number;
  agencyId: string;
}

export type AlertSeverity = 'INFO' | 'WARNING' | 'CRITICAL';

export interface Alert {
  id: string;
  vehicleId: string;
  type: string; // e.g. "OVERSPEED", "GEOFENCE_EXIT"
  severity: AlertSeverity;
  message: string;
  timestamp: string;
  resolved: boolean;
}

// Transit Network Observability Types
export interface Stop {
  stopId: string;
  name: string;
  latitude: number;
  longitude: number;
}

export interface StopTime {
  tripId: string;
  stopId: string;
  arrivalTime: string; // HH:MM:SS
  departureTime: string; // HH:MM:SS
  stopSequence: number;
}

export interface DelayUpdateEvent {
  vehicleId: string;
  tripId: string;
  latitude: number;
  longitude: number;
  recordedAt: string;
  delaySeconds: number;
}

export interface EdgeWeight {
  sourceStopId: string;
  targetStopId: string;
  tripId: string;
  scheduledDurationSeconds: number;
  liveDelaySeconds: number;
  weightSeconds: number;
  lastUpdated: string;
}
