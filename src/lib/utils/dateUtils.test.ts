import { describe, expect, it } from 'vitest';
import { formatDate, formatRelativeTime, isToday, isValidDate } from './dateUtils';

describe('formatDate', () => {
  it('returns dd/mm/yy for a valid timestamp', () => {
    // 2024-03-15 12:00:00 UTC
    const ts = new Date('2024-03-15T12:00:00Z').getTime();
    const result = formatDate(ts);
    // Should contain day/month/year in some locale format
    expect(result).toMatch(/\d{2}\/\d{2}\/\d{2}/);
  });

  it('returns "No date" for 0', () => {
    expect(formatDate(0)).toBe('No date');
  });

  it('returns "No date" for NaN', () => {
    expect(formatDate(NaN)).toBe('No date');
  });

  it('returns "No date" for undefined', () => {
    expect(formatDate(undefined as unknown as number)).toBe('No date');
  });
});

describe('formatRelativeTime', () => {
  it('returns "Just now" for a timestamp less than 60 seconds ago', () => {
    const ts = Date.now() - 30_000;
    expect(formatRelativeTime(ts)).toBe('Just now');
  });

  it('returns minutes ago for recent timestamps', () => {
    const ts = Date.now() - 5 * 60 * 1000;
    expect(formatRelativeTime(ts)).toBe('5 minutes ago');
  });

  it('returns hours ago for timestamps within today', () => {
    const ts = Date.now() - 3 * 60 * 60 * 1000;
    expect(formatRelativeTime(ts)).toBe('3 hours ago');
  });

  it('returns "Yesterday" for 1 day ago', () => {
    const ts = Date.now() - 25 * 60 * 60 * 1000;
    expect(formatRelativeTime(ts)).toBe('Yesterday');
  });

  it('returns days ago for 2-6 days', () => {
    const ts = Date.now() - 4 * 24 * 60 * 60 * 1000;
    expect(formatRelativeTime(ts)).toBe('4 days ago');
  });

  it('returns formatted date for older timestamps', () => {
    const ts = Date.now() - 30 * 24 * 60 * 60 * 1000;
    const result = formatRelativeTime(ts);
    expect(result).toMatch(/\d{2}\/\d{2}\/\d{2}/);
  });

  it('returns "No date" for invalid input', () => {
    expect(formatRelativeTime(0)).toBe('No date');
    expect(formatRelativeTime(NaN)).toBe('No date');
  });
});

describe('isValidDate', () => {
  it('returns true for a valid timestamp', () => {
    expect(isValidDate(Date.now())).toBe(true);
  });

  it('returns false for 0', () => {
    expect(isValidDate(0)).toBe(false);
  });

  it('returns false for NaN', () => {
    expect(isValidDate(NaN)).toBe(false);
  });
});

describe('isToday', () => {
  it('returns true for current timestamp', () => {
    expect(isToday(Date.now())).toBe(true);
  });

  it('returns false for yesterday', () => {
    const yesterday = Date.now() - 48 * 60 * 60 * 1000;
    expect(isToday(yesterday)).toBe(false);
  });

  it('handles seconds-precision timestamps (auto-converts)', () => {
    // Unix seconds (not milliseconds) - isToday multiplies by 1000
    const nowSeconds = Math.floor(Date.now() / 1000);
    expect(isToday(nowSeconds)).toBe(true);
  });

  it('returns false for invalid input', () => {
    expect(isToday(0)).toBe(false);
    expect(isToday(NaN)).toBe(false);
  });
});
