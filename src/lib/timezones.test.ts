import { describe, expect, it } from 'vitest';
import { localTimezone, timezoneNames, timezoneOffsetLabel } from './timezones';

describe('timezones', () => {
  it('lists UTC and the local zone, sorted and without repeats', () => {
    const names = timezoneNames();
    expect(names).toContain('UTC');
    expect(names).toContain(localTimezone());
    expect(new Set(names).size).toBe(names.length);
    expect([...names].sort((a, b) => a.localeCompare(b))).toEqual(names);
  });

  it('labels the offset of a zone at a given instant', () => {
    const at = new Date('2026-11-07T20:27:15Z');
    expect(timezoneOffsetLabel('America/Sao_Paulo', at)).toBe('UTC−03:00');
    expect(timezoneOffsetLabel('Asia/Kolkata', at)).toBe('UTC+05:30');
    expect(timezoneOffsetLabel('UTC', at)).toBe('UTC±00:00');
  });

  it('returns null for a name the system does not know', () => {
    expect(timezoneOffsetLabel('Not/AZone')).toBeNull();
  });
});
