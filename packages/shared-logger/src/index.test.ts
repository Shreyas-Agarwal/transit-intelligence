import { describe, it, expect, vi, afterEach } from 'vitest';
import { Logger } from './index';

describe('Shared Logger', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should format info messages as JSON with correct metadata', () => {
    const logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
    const logger = new Logger('TestService');
    logger.info('Hello world', { key: 'value' });

    expect(logSpy).toHaveBeenCalled();
    const output = JSON.parse(logSpy.mock.calls[0][0] as string);
    expect(output.service).toBe('TestService');
    expect(output.level).toBe('INFO');
    expect(output.message).toBe('Hello world');
    expect(output.key).toBe('value');
    expect(output.timestamp).toBeDefined();
  });

  it('should format warn messages correctly', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const logger = new Logger('TestService');
    logger.warn('Something is fishy');

    expect(warnSpy).toHaveBeenCalled();
    const output = JSON.parse(warnSpy.mock.calls[0][0] as string);
    expect(output.level).toBe('WARN');
    expect(output.message).toBe('Something is fishy');
  });

  it('should format errors with stack trace details', () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const logger = new Logger('TestService');
    const err = new Error('Database connection failed');
    logger.error('Failed to start database', err, { contextId: 42 });

    expect(errorSpy).toHaveBeenCalled();
    const output = JSON.parse(errorSpy.mock.calls[0][0] as string);
    expect(output.level).toBe('ERROR');
    expect(output.message).toBe('Failed to start database');
    expect(output.error).toBe('Database connection failed');
    expect(output.stack).toBeDefined();
    expect(output.contextId).toBe(42);
  });
});
