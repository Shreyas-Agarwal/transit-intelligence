import { describe, it, expect } from 'vitest';
import {
  BaseError,
  NotFoundError,
  ValidationError,
  UnauthorizedError,
  ForbiddenError,
  InternalServerError,
} from './index';

describe('Shared Errors', () => {
  it('should instantiate BaseError with correct properties', () => {
    const error = new BaseError('Custom message', 418, 'TEAPOT', { custom: 'info' });
    expect(error.message).toBe('Custom message');
    expect(error.statusCode).toBe(418);
    expect(error.errorCode).toBe('TEAPOT');
    expect(error.details).toEqual({ custom: 'info' });
    expect(error).toBeInstanceOf(Error);
    expect(error).toBeInstanceOf(BaseError);
  });

  it('should instantiate NotFoundError with default values', () => {
    const error = new NotFoundError();
    expect(error.message).toBe('Resource not found');
    expect(error.statusCode).toBe(404);
    expect(error.errorCode).toBe('NOT_FOUND');
    expect(error).toBeInstanceOf(BaseError);
  });

  it('should instantiate ValidationError with details', () => {
    const details = { field: 'email', reason: 'invalid format' };
    const error = new ValidationError('Bad request', details);
    expect(error.message).toBe('Bad request');
    expect(error.statusCode).toBe(400);
    expect(error.errorCode).toBe('VALIDATION_ERROR');
    expect(error.details).toBe(details);
    expect(error).toBeInstanceOf(BaseError);
  });

  it('should instantiate UnauthorizedError', () => {
    const error = new UnauthorizedError();
    expect(error.statusCode).toBe(401);
    expect(error.errorCode).toBe('UNAUTHORIZED');
  });

  it('should instantiate ForbiddenError', () => {
    const error = new ForbiddenError();
    expect(error.statusCode).toBe(403);
    expect(error.errorCode).toBe('FORBIDDEN');
  });

  it('should instantiate InternalServerError', () => {
    const error = new InternalServerError();
    expect(error.statusCode).toBe(500);
    expect(error.errorCode).toBe('INTERNAL_ERROR');
  });
});
