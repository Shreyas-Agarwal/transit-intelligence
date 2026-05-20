import express from 'express';
import cors from 'cors';
import { Logger } from '@transit-intelligence/shared-logger';
import { NotFoundError, BaseError } from '@transit-intelligence/shared-errors';
import { Vehicle } from '@transit-intelligence/shared-types';

const app = express();
const port = process.env.PORT || 3000;
const logger = new Logger('API-Service');

app.use(cors());
app.use(express.json());

// Sample active vehicles state
const mockVehicles: Vehicle[] = [
  { id: 'v1', licensePlate: 'TX-1234', status: 'ACTIVE', capacity: 40, agencyId: 'agency-1' },
  { id: 'v2', licensePlate: 'TX-5678', status: 'ACTIVE', capacity: 55, agencyId: 'agency-1' },
];

app.get('/api/v1/vehicles/active', (req, res) => {
  logger.info('Fetching active vehicles list', { requestId: 'req-' + Date.now() });
  res.json(mockVehicles);
});

// Error handling middleware fallback
app.use((req, res, next) => {
  next(new NotFoundError(`Route ${req.method} ${req.path} not found`));
});

// Global error handler
app.use((err: Error, req: express.Request, res: express.Response, _next: express.NextFunction) => {
  if (err instanceof BaseError) {
    logger.warn(err.message, { statusCode: err.statusCode, errorCode: err.errorCode });
    res.status(err.statusCode).json({
      error: err.message,
      code: err.errorCode,
      details: err.details,
    });
  } else {
    logger.error('Unhandled internal server error', err);
    res.status(500).json({
      error: 'An internal server error occurred',
      code: 'INTERNAL_ERROR',
    });
  }
});

app.listen(port, () => {
  logger.info(`Express API gateway listening at http://localhost:${port}`);
});
