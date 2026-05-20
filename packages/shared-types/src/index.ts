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
