import * as duckdb from '@duckdb/duckdb-wasm';

import duckdb_wasm_eh from '@duckdb/duckdb-wasm/dist/duckdb-eh.wasm?url';
import duckdb_wasm from '@duckdb/duckdb-wasm/dist/duckdb-mvp.wasm?url';

import worker_eh from '@duckdb/duckdb-wasm/dist/duckdb-browser-eh.worker.js?url';
import worker_mvp from '@duckdb/duckdb-wasm/dist/duckdb-browser-mvp.worker.js?url';

let db: duckdb.AsyncDuckDB | null = null;

export async function getDuckDB() {
  if (db) {
    return db;
  }

  const bundles: duckdb.DuckDBBundles = {
    mvp: {
      mainModule: duckdb_wasm,
      mainWorker: worker_mvp,
    },
    eh: {
      mainModule: duckdb_wasm_eh,
      mainWorker: worker_eh,
    },
  };

  const bundle = await duckdb.selectBundle(bundles);

  const worker = new Worker(bundle.mainWorker!);

  const logger = new duckdb.ConsoleLogger();

  db = new duckdb.AsyncDuckDB(logger, worker);

  await db.instantiate(bundle.mainModule, bundle.pthreadWorker);

  return db;
}

export async function registerParquetFile(db: duckdb.AsyncDuckDB, fileName: string) {
  const response = await fetch(`/data/${fileName}`);

  if (!response.ok) {
    throw new Error(`Failed to load ${fileName}`);
  }

  const buffer = await response.arrayBuffer();

  await db.registerFileBuffer(fileName, new Uint8Array(buffer));
}
