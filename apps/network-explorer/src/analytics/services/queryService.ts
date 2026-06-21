import type * as duckdb from '@duckdb/duckdb-wasm';
import type { QueryResult } from '../registry/types';

//-------------------------------------
// Analytics Workbench — Query Execution Service
//
// Executes a pre-generated SQL string against the shared DuckDB connection,
// measures wall-clock execution time, and normalises the Arrow result to a
// plain JS object array for downstream consumption.
//
// QueryResult is defined in registry/types.ts and re-exported here for
// backward compatibility with any existing imports.
//-------------------------------------

// Re-export so existing consumers can still import from this module.
export type { QueryResult };

/**
 * Execute a SQL query against the provided DuckDB connection and return
 * normalised results with performance metrics.
 *
 * @param sql  - Query string (from workbenchGenerator or legacyGenerator)
 * @param conn - Shared AsyncDuckDBConnection (from `lib/connection.ts`)
 */
export async function executeAnalyticsQuery(
  sql: string,
  conn: duckdb.AsyncDuckDBConnection,
): Promise<QueryResult> {
  const start = performance.now();

  const arrowResult = await conn.query(sql);

  const executionMs = performance.now() - start;

  // Convert Apache Arrow table to plain JS objects.
  // toArray() returns Arrow struct proxies; spreading each row into a plain
  // object ensures downstream code (chart builder, React) never needs to
  // understand Arrow internals.
  const rows: Record<string, unknown>[] = arrowResult
    .toArray()
    .map((row) => ({ ...row }));

  return {
    rows,
    executionMs,
    rowCount: rows.length,
  };
}
