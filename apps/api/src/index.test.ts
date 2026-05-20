import { describe, it, expect } from 'vitest';
import request from 'supertest';
import { app } from './index';

// Set NODE_ENV to test to prevent listener
process.env.NODE_ENV = 'test';

describe('Express API Gateway', () => {
  it('GET /api/v1/vehicles/active should return mock active vehicles', async () => {
    const res = await request(app)
      .get('/api/v1/vehicles/active')
      .expect('Content-Type', /json/)
      .expect(200); // Expect success status code 200

    expect(res.body).toBeInstanceOf(Array);
    expect(res.body.length).toBeGreaterThan(0);
    expect(res.body[0]).toHaveProperty('id');
    expect(res.body[0]).toHaveProperty('licensePlate');
  });

  it('GET /api/v1/routing/calculate should return mock route pathfinding fallback', async () => {
    const res = await request(app)
      .get('/api/v1/routing/calculate?from=stop-A&to=stop-B')
      .expect('Content-Type', /json/)
      .expect(200);

    expect(res.body).toHaveProperty('route');
    expect(res.body.route).toEqual(['stop-A', 'stop-B']);
    expect(res.body).toHaveProperty('status', 'MOCK_FALLBACK');
  });

  it('GET /api/v1/network/health should return mock telemetry health fallback', async () => {
    const res = await request(app)
      .get('/api/v1/network/health')
      .expect('Content-Type', /json/)
      .expect(200);

    expect(res.body).toHaveProperty('status', 'MOCK_FALLBACK');
    expect(res.body).toHaveProperty('hotspots');
  });

  it('GET /api/v1/non-existent-route should return 404', async () => {
    const res = await request(app).get('/api/v1/non-existent-route').expect(404);

    expect(res.body).toHaveProperty('error');
    expect(res.body.code).toBe('NOT_FOUND');
  });
});
