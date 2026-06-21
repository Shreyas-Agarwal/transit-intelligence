import type { BenchmarkDefinition } from '../types';

export const overviewBenchmarks: BenchmarkDefinition[] = [
  {
    name: 'count_stops',
    query: `
      SELECT COUNT(*)
      FROM stops
    `,
  },
  {
    name: 'count_routes',
    query: `
      SELECT COUNT(*)
      FROM routes
    `,
  },
  {
    name: 'count_trips',
    query: `
      SELECT COUNT(*)
      FROM trips
    `,
  },
  {
    name: 'count_stop_times',
    query: `
      SELECT COUNT(*)
      FROM stop_times
    `,
  },
];
