import { Pool, type PoolConfig } from 'pg';
import { Database } from 'duckdb';
import { createClient, type ClickHouseClient } from '@clickhouse/client';

export class DatabaseManager {
  private pgPool: Pool | null = null;
  private duckDb: Database | null = null;
  private clickHouseClient: ClickHouseClient | null = null;

  /**
   * Initializes and returns the PostgreSQL connection pool.
   * If config is provided, it overrides environment defaults.
   */
  public getPostgresPool(config?: PoolConfig): Pool {
    if (!this.pgPool) {
      const defaultConfig: PoolConfig = {
        connectionString:
          process.env.DATABASE_URL ||
          'postgresql://postgres:postgres@localhost:5432/transit_intelligence',
        max: parseInt(process.env.PG_POOL_MAX || '10', 10),
        idleTimeoutMillis: 30000,
        connectionTimeoutMillis: 2000,
      };

      this.pgPool = new Pool({ ...defaultConfig, ...config });
    }
    return this.pgPool;
  }

  /**
   * Initializes and returns the embedded DuckDB connection instance.
   * Path defaults to an in-memory DB unless DUCKDB_PATH or filePath is specified.
   */
  public getDuckDb(filePath?: string): Database {
    if (!this.duckDb) {
      const path = filePath || process.env.DUCKDB_PATH || ':memory:';
      this.duckDb = new Database(path);
    }
    return this.duckDb;
  }

  /**
   * Initializes and returns the ClickHouse OLAP client.
   * Connection configuration merges environment variables and optional parameters.
   */
  public getClickHouseClient(config?: Parameters<typeof createClient>[0]): ClickHouseClient {
    if (!this.clickHouseClient) {
      const url =
        process.env.CLICKHOUSE_URL ||
        process.env.CLICKHOUSE_HOST ||
        'http://localhost:8123';
      const username = process.env.CLICKHOUSE_USER || 'default';
      const password = process.env.CLICKHOUSE_PASSWORD || '';
      const database = process.env.CLICKHOUSE_DB || 'default';

      this.clickHouseClient = createClient({
        url,
        username,
        password,
        database,
        clickhouse_settings: {
          max_execution_time: 30,
        },
        ...config,
      });
    }
    return this.clickHouseClient;
  }

  /**
   * Shuts down all active database connections gracefully.
   */
  public async closeAll(): Promise<void> {
    const promises: Promise<unknown>[] = [];

    if (this.pgPool) {
      const pool = this.pgPool;
      this.pgPool = null;
      promises.push(pool.end());
    }

    if (this.clickHouseClient) {
      const client = this.clickHouseClient;
      this.clickHouseClient = null;
      promises.push(client.close());
    }

    if (this.duckDb) {
      const db = this.duckDb;
      this.duckDb = null;
      promises.push(
        new Promise<void>((resolve, reject) => {
          db.close((err) => {
            if (err) {
              reject(err);
            } else {
              resolve();
            }
          });
        }),
      );
    }

    await Promise.all(promises);
  }
}

export const dbManager = new DatabaseManager();
