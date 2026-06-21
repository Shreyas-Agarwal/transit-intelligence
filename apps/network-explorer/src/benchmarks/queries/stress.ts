import type { BenchmarkDefinition } from '../types';

export const stressBenchmarks: BenchmarkDefinition[] = [
  {
    name: 'full_stop_times_scan',
    query: `
      SELECT *
      FROM stop_times
      LIMIT 100000
    `,
  },
  {
    name: 'global_stop_times_grouping',
    query: `
      SELECT
          stop_id,
          COUNT(*)
      FROM stop_times
      GROUP BY stop_id
      ORDER BY COUNT(*) DESC
    `,
  },
];
