import type * as duckdb from '@duckdb/duckdb-wasm';

import { joinBenchmarks } from '../queries/joins';
import { runBenchmark } from '../runner';

export async function runJoinSuite(conn: duckdb.AsyncDuckDBConnection) {
  return Promise.all(joinBenchmarks.map((benchmark) => runBenchmark(conn, benchmark)));
}
