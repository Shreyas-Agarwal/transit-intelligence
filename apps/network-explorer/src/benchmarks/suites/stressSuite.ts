import type * as duckdb from '@duckdb/duckdb-wasm';

import { stressBenchmarks } from '../queries/stress';
import { runBenchmark } from '../runner';

export async function runStressSuite(conn: duckdb.AsyncDuckDBConnection) {
  return Promise.all(stressBenchmarks.map((benchmark) => runBenchmark(conn, benchmark)));
}
