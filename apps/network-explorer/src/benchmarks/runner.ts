import type * as duckdb from '@duckdb/duckdb-wasm';
import type { BenchmarkDefinition, BenchmarkResult } from './types';

export async function runBenchmark(
  conn: duckdb.AsyncDuckDBConnection,
  benchmark: BenchmarkDefinition,
): Promise<BenchmarkResult> {
  const coldStart = performance.now();

  await conn.query(benchmark.query);

  const coldEnd = performance.now();

  const warmStart = performance.now();

  await conn.query(benchmark.query);

  const warmEnd = performance.now();

  return {
    name: benchmark.name,
    coldMs: coldEnd - coldStart,
    warmMs: warmEnd - warmStart,
  };
}
