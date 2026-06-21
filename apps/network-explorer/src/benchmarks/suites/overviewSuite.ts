import type * as duckdb from '@duckdb/duckdb-wasm';

import { overviewBenchmarks } from '../queries/overview';
import { runBenchmark } from '../runner';

export async function runOverviewSuite(conn: duckdb.AsyncDuckDBConnection) {
  return Promise.all(overviewBenchmarks.map((benchmark) => runBenchmark(conn, benchmark)));
}
