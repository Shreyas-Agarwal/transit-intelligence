import type * as duckdb from '@duckdb/duckdb-wasm';
import type { DiscoveredTable, DiscoveredColumn } from '../registry/types';
import { classifyColumn } from './columnClassifier';

//-------------------------------------
// Analytics Workbench — Catalog Discovery Service
//
// Queries the DuckDB information_schema to discover all tables and views in
// the 'main' schema and classifies their columns automatically.
//
// Source of truth: DuckDB catalog (not the filesystem, not a static registry).
// Any view or table registered in DuckDB automatically appears in the UI.
//-------------------------------------

/**
 * Discover all tables and views in the DuckDB 'main' schema, including their
 * columns and automatically classified kinds.
 *
 * Execution plan:
 *   1. SELECT table_name FROM information_schema.tables WHERE table_schema = 'main'
 *   2. For each table: SELECT column_name, data_type FROM information_schema.columns
 *   3. classifyColumn() for each column
 *
 * Tables are returned alphabetically. Columns retain their ordinal position.
 *
 * @param conn - Shared AsyncDuckDBConnection
 * @returns    - Ordered array of DiscoveredTable (tables + views combined)
 */
export async function discoverCatalog(
  conn: duckdb.AsyncDuckDBConnection,
): Promise<DiscoveredTable[]> {
  // ── Step 1: Enumerate all tables/views in the main schema ──────────────────
  const tablesResult = await conn.query(`
    SELECT table_name
    FROM information_schema.tables
    WHERE table_schema = 'main'
    ORDER BY table_name
  `);

  const tableNames: string[] = tablesResult
    .toArray()
    .map((row) => String(({ ...row }).table_name ?? ''))
    .filter(Boolean);

  // ── Step 2: Fetch columns for each table concurrently ─────────────────────
  const tables: DiscoveredTable[] = await Promise.all(
    tableNames.map(async (name): Promise<DiscoveredTable> => {
      const colResult = await conn.query(`
        SELECT column_name, data_type
        FROM information_schema.columns
        WHERE table_schema = 'main'
          AND table_name = '${name}'
        ORDER BY ordinal_position
      `);

      const columns: DiscoveredColumn[] = colResult
        .toArray()
        .map((row) => {
          const r = { ...row } as { column_name: string; data_type: string };
          return {
            name: r.column_name,
            dataType: r.data_type,
            kind: classifyColumn(r.column_name, r.data_type),
          };
        });

      return { name, columns };
    }),
  );

  return tables;
}
