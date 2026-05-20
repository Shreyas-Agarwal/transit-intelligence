export class BaseError extends Error {
  public statusCode: number;
  public errorCode: string;
  public details?: unknown;

  constructor(message: string, statusCode: number, errorCode: string, details?: unknown) {
    super(message);
    this.statusCode = statusCode;
    this.errorCode = errorCode;
    this.details = details;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class NotFoundError extends BaseError {
  constructor(message = 'Resource not found', errorCode = 'NOT_FOUND') {
    super(message, 404, errorCode);
  }
}

export class ValidationError extends BaseError {
  constructor(message = 'Validation failed', details?: unknown, errorCode = 'VALIDATION_ERROR') {
    super(message, 400, errorCode, details);
  }
}

export class UnauthorizedError extends BaseError {
  constructor(message = 'Unauthorized access', errorCode = 'UNAUTHORIZED') {
    super(message, 401, errorCode);
  }
}

export class ForbiddenError extends BaseError {
  constructor(message = 'Forbidden access', errorCode = 'FORBIDDEN') {
    super(message, 403, errorCode);
  }
}

export class InternalServerError extends BaseError {
  constructor(message = 'Internal server error', errorCode = 'INTERNAL_ERROR') {
    super(message, 500, errorCode);
  }
}
