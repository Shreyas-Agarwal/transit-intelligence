import type * as duckdb from '@duckdb/duckdb-wasm';

//-------------------------------------
// Shared DuckDB Connection Singleton
//
// App.tsx establishes the connection during initialization and registers it
// here. All downstream consumers (benchmarks, Analytics Studio) read from
// this module rather than opening their own connections. The connection is
// kept open for the lifetime of the application.
//-------------------------------------

let sharedConn: duckdb.AsyncDuckDBConnection | null = null;

export function setSharedConnection(conn: duckdb.AsyncDuckDBConnection): void {
  sharedConn = conn;
}

export function getSharedConnection(): duckdb.AsyncDuckDBConnection {
  if (!sharedConn) {
    throw new Error(
      'DuckDB connection is not initialized. ' +
        'Ensure setSharedConnection() has been called before accessing analytics.',
    );
  }
  return sharedConn;
}
