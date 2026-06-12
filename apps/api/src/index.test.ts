import { describe, it, expect } from 'vitest';

// Set NODE_ENV to test to prevent listener before importing app
process.env.NODE_ENV = 'test';

import { app } from './index';

describe('Fastify API Gateway', () => {
  it('GET /api/v1/vehicles/active should return mock active vehicles', async () => {
    const res = await app.inject({
      method: 'GET',
      url: '/api/v1/vehicles/active',
    });

    expect(res.statusCode).toBe(200);
    expect(res.headers['content-type']).toMatch(/json/);

    const body = JSON.parse(res.payload);
    expect(body).toBeInstanceOf(Array);
    expect(body.length).toBeGreaterThan(0);
    expect(body[0]).toHaveProperty('id');
    expect(body[0]).toHaveProperty('licensePlate');
  });

  it('GET /api/v1/routing/calculate should return mock route pathfinding fallback', async () => {
    const res = await app.inject({
      method: 'GET',
      url: '/api/v1/routing/calculate?from=stop-A&to=stop-B',
    });

    expect(res.statusCode).toBe(200);
    expect(res.headers['content-type']).toMatch(/json/);

    const body = JSON.parse(res.payload);
    expect(body).toHaveProperty('route');
    expect(body.route).toEqual(['stop-A', 'stop-B']);
    expect(body).toHaveProperty('status', 'MOCK_FALLBACK');
  });

  it('GET /api/v1/network/health should return mock telemetry health fallback', async () => {
    const res = await app.inject({
      method: 'GET',
      url: '/api/v1/network/health',
    });

    expect(res.statusCode).toBe(200);
    expect(res.headers['content-type']).toMatch(/json/);

    const body = JSON.parse(res.payload);
    expect(body).toHaveProperty('status', 'MOCK_FALLBACK');
    expect(body).toHaveProperty('hotspots');
  });

  it('GET /api/v1/non-existent-route should return 404', async () => {
    const res = await app.inject({
      method: 'GET',
      url: '/api/v1/non-existent-route',
    });

    expect(res.statusCode).toBe(404);
    const body = JSON.parse(res.payload);
    expect(body).toHaveProperty('error');
    expect(body.code).toBe('NOT_FOUND');
  });
});
