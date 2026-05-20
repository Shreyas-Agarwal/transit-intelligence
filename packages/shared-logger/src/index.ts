export interface LogContext {
  requestId?: string;
  correlationId?: string;
  [key: string]: unknown;
}

export class Logger {
  private serviceName: string;

  constructor(serviceName: string) {
    this.serviceName = serviceName;
  }

  private formatMessage(level: string, message: string, context?: LogContext) {
    return JSON.stringify({
      timestamp: new Date().toISOString(),
      service: this.serviceName,
      level,
      message,
      ...context,
    });
  }

  info(message: string, context?: LogContext) {
    console.log(this.formatMessage('INFO', message, context));
  }

  warn(message: string, context?: LogContext) {
    console.warn(this.formatMessage('WARN', message, context));
  }

  error(message: string, error?: unknown, context?: LogContext) {
    const errDetails =
      error instanceof Error
        ? { error: error.message, stack: error.stack }
        : { error: String(error) };
    console.error(this.formatMessage('ERROR', message, { ...errDetails, ...context }));
  }
}
