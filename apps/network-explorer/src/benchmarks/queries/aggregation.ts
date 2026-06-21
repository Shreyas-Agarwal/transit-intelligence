import type { BenchmarkDefinition } from '../types';

export const aggregationBenchmarks: BenchmarkDefinition[] = [
  {
    name: 'route_trip_counts',
    query: `
      SELECT
          route_id,
          COUNT(*) AS trip_count
      FROM trips
      GROUP BY route_id
    `,
  },
  {
    name: 'stop_utilization',
    query: `
      SELECT
          stop_id,
          COUNT(*) AS visits
      FROM stop_times
      GROUP BY stop_id
    `,
  },
  {
    name: 'trip_length_distribution',
    query: `
      SELECT
          trip_id,
          COUNT(*) AS stop_count
      FROM stop_times
      GROUP BY trip_id
    `,
  },
];
