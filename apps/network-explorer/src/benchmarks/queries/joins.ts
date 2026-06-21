import type { BenchmarkDefinition } from '../types';

export const joinBenchmarks: BenchmarkDefinition[] = [
  {
    name: 'route_stop_events',
    query: `
      SELECT
          t.route_id,
          COUNT(*) AS stop_events
      FROM stop_times st
      JOIN trips t
          ON st.trip_id = t.trip_id
      GROUP BY t.route_id
    `,
  },
  {
    name: 'agency_route_trip_summary',
    query: `
      SELECT
          a.agency_name,
          COUNT(DISTINCT r.route_id) AS route_count,
          COUNT(DISTINCT t.trip_id) AS trip_count
      FROM agencies a
      JOIN routes r
          ON a.agency_id = r.agency_id
      JOIN trips t
          ON r.route_id = t.route_id
      GROUP BY a.agency_name
    `,
  },
];
