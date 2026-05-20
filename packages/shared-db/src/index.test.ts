import { describe, it, expect, afterEach } from 'vitest';
import { DatabaseManager, dbManager } from './index';
import { Pool } from 'pg';
import { Database } from 'duckdb';

describe('Shared DB Manager', () => {
  afterEach(async () => {
    await dbManager.closeAll();
  });

  it('should create and return a Postgres Pool instance', async () => {
    const manager = new DatabaseManager();
    const pool = manager.getPostgresPool();
    expect(pool).toBeInstanceOf(Pool);
    await manager.closeAll();
  });

  it('should initialize and return a DuckDB Database instance', async () => {
    const manager = new DatabaseManager();
    const db = manager.getDuckDb(':memory:');
    expect(db).toBeInstanceOf(Database);

    // Test a simple query to verify it works
    await new Promise<void>((resolve, reject) => {
      db.all('SELECT 1 + 1 as result', (err, rows) => {
        try {
          expect(err).toBeNull();
          expect(rows).toEqual([{ result: 2 }]);
          resolve();
        } catch (e) {
          reject(e);
        }
      });
    });

    await manager.closeAll();
  });

  it('should initialize and return a ClickHouse Client instance', async () => {
    const manager = new DatabaseManager();
    const client = manager.getClickHouseClient({
      url: 'http://localhost:8123',
    });
    expect(client).toBeDefined();
    expect(client.query).toBeDefined();
    await manager.closeAll();
  });

  it('should maintain singletons for each driver connection', async () => {
    const manager = new DatabaseManager();
    const db1 = manager.getDuckDb(':memory:');
    const db2 = manager.getDuckDb(':memory:');
    expect(db1).toBe(db2);

    const pool1 = manager.getPostgresPool();
    const pool2 = manager.getPostgresPool();
    expect(pool1).toBe(pool2);

    const client1 = manager.getClickHouseClient();
    const client2 = manager.getClickHouseClient();
    expect(client1).toBe(client2);

    await manager.closeAll();
  });
});
