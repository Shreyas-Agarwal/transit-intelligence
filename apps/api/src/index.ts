import Fastify from 'fastify';
import cors from '@fastify/cors';
import { Logger } from '@transit-intelligence/shared-logger';
import { NotFoundError, BaseError } from '@transit-intelligence/shared-errors';
import { Vehicle } from '@transit-intelligence/shared-types';

const app = Fastify({ logger: false });
const port = process.env.PORT || 3000;
const logger = new Logger('API-Service');

app.register(cors);

import path from 'path';
import fs from 'fs';
import { dbManager } from '@transit-intelligence/shared-db';
import { EdgeWeight } from '@transit-intelligence/shared-types';

// Resolve DuckDB database path
const defaultDbPath = path.resolve(__dirname, '../../workers/analytics.db');
const dbPath =
  process.env.DUCKDB_PATH || (fs.existsSync(defaultDbPath) ? defaultDbPath : ':memory:');

logger.info(`Connecting to DuckDB database at: ${dbPath}`);
const duckDb = dbManager.getDuckDb(dbPath);

// Helper to query DuckDB asynchronously
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function queryDuckDb<T>(sql: string, params: any[] = []): Promise<T[]> {
  return new Promise((resolve, reject) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    duckDb.all(sql, ...params, (err: any, rows: any) => {
      if (err) {
        reject(err);
      } else {
        resolve(rows as T[]);
      }
    });
  });
}

// Sample active vehicles state fallback
const mockVehicles: Vehicle[] = [
  { id: 'v1', licensePlate: 'TX-1234', status: 'ACTIVE', capacity: 40, agencyId: 'agency-1' },
  { id: 'v2', licensePlate: 'TX-5678', status: 'ACTIVE', capacity: 55, agencyId: 'agency-1' },
];

app.get('/api/v1/vehicles/active', async (_request, _reply) => {
  logger.info('Fetching active vehicles list', { requestId: 'req-' + Date.now() });
  return mockVehicles;
});

// Dynamic Route pathfinding weight calculator (queries temporal graph edge weights)
app.get('/api/v1/routing/calculate', async (request, _reply) => {
  const { from, to } = request.query as { from?: string; to?: string };
  logger.info(`Calculating dynamic routing from ${from} to ${to}`);

  // Check if edge_weights table exists in DuckDB
  const tables = await queryDuckDb<{ name: string }>(
    "SELECT name FROM sqlite_master WHERE type='table' AND name='edge_weights'",
  );

  if (tables.length === 0) {
    // Fallback if worker hasn't run yet
    return {
      route: [from, to],
      totalDurationSeconds: 180,
      liveDelaySeconds: 0,
      status: 'MOCK_FALLBACK',
      message: 'DuckDB tables not yet populated by workers. Showing fallback weights.',
    };
  }

  const weights = await queryDuckDb<EdgeWeight>(
    'SELECT * FROM edge_weights WHERE source_stop_id = ? AND target_stop_id = ?',
    [from, to],
  );

  return {
    from,
    to,
    weights,
    status: 'LIVE_ANALYTICS',
  };
});

// Network congestion and delay hotspots analytics endpoint
app.get('/api/v1/network/health', async (_request, _reply) => {
  logger.info('Fetching network congestion profiles');

  const tables = await queryDuckDb<{ name: string }>(
    "SELECT name FROM sqlite_master WHERE type='table' AND name='vehicle_positions'",
  );

  if (tables.length === 0) {
    return {
      hotspots: [],
      status: 'MOCK_FALLBACK',
      message: 'No live telemetry updates recorded yet.',
    };
  }

  const hotspots = await queryDuckDb<{ trip_id: string; avg_delay: number; pings_count: number }>(
    `SELECT 
       trip_id, 
       AVG(delay_seconds) as avg_delay, 
       COUNT(*) as pings_count 
     FROM vehicle_positions 
     GROUP BY trip_id 
     ORDER BY avg_delay DESC`,
  );

  return {
    hotspots,
    status: 'LIVE_ANALYTICS',
  };
});

// Error handling middleware fallback (404)
app.setNotFoundHandler((request, _reply) => {
  const pathPart = request.url.split('?')[0];
  throw new NotFoundError(`Route ${request.method} ${pathPart} not found`);
});

// Global error handler
app.setErrorHandler((error, request, reply) => {
  if (error instanceof BaseError) {
    logger.warn(error.message, { statusCode: error.statusCode, errorCode: error.errorCode });
    reply.status(error.statusCode).send({
      error: error.message,
      code: error.errorCode,
      details: error.details,
    });
  } else {
    logger.error('Unhandled internal server error', error);
    reply.status(500).send({
      error: 'An internal server error occurred',
      code: 'INTERNAL_ERROR',
    });
  }
});

if (process.env.NODE_ENV !== 'test') {
  app.listen({ port: Number(port), host: '0.0.0.0' }, (err, address) => {
    if (err) {
      logger.error('Failed to start Fastify server', err);
      process.exit(1);
    }
    logger.info(`Fastify API gateway listening at ${address}`);
  });
}

export { app };
