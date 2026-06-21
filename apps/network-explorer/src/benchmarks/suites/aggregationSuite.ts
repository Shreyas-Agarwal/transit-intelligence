import type * as duckdb from '@duckdb/duckdb-wasm';

import { aggregationBenchmarks } from '../queries/aggregation';
import { runBenchmark } from '../runner';

export async function runAggregationSuite(conn: duckdb.AsyncDuckDBConnection) {
  return Promise.all(aggregationBenchmarks.map((benchmark) => runBenchmark(conn, benchmark)));
}
