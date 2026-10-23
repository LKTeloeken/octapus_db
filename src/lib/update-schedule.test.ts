import { describe, expect, it } from 'vitest';
import { compareVersions, isCheckDue } from './update-schedule';

const HOUR = 60 * 60_000;

describe('isCheckDue', () => {
  const base = {
    now: 10 * HOUR,
    lastCheckedAt: 8 * HOUR,
    status: 'idle' as const,
    checking: false,
    minIntervalMs: 4 * HOUR,
  };

  it('checa na primeira vez', () => {
    expect(isCheckDue({ ...base, lastCheckedAt: null })).toBe(true);
  });

  it('espera o intervalo desde a última checagem', () => {
    expect(isCheckDue(base)).toBe(false);
    expect(isCheckDue({ ...base, now: 12 * HOUR })).toBe(true);
  });

  it('o relógio vale mesmo depois de uma suspensão longa', () => {
    expect(isCheckDue({ ...base, now: 100 * HOUR })).toBe(true);
  });

  it('não checa com uma checagem em andamento', () => {
    expect(isCheckDue({ ...base, now: 12 * HOUR, checking: true })).toBe(false);
  });

  it('segue checando com um update disponível', () => {
    expect(isCheckDue({ ...base, now: 12 * HOUR, status: 'available' })).toBe(
      true,
    );
  });

  it('não checa baixando nem com o update à espera do reinício', () => {
    for (const status of ['downloading', 'ready'] as const) {
      expect(isCheckDue({ ...base, lastCheckedAt: null, status })).toBe(false);
    }
  });
});

describe('compareVersions', () => {
  it.each([
    ['1.0.0', '1.0.0', 0],
    ['1.0.1', '1.0.0', 1],
    ['1.1.0', '1.0.9', 1],
    ['2.0.0', '10.0.0', -1],
    ['v1.2.3', '1.2.3', 0],
    ['1.1.0', '1.1.0-beta.14', 1],
    ['1.1.0-beta.14', '1.1.0-beta.13', 1],
    ['1.1.0-beta.2', '1.1.0-beta.10', -1],
    ['1.1.0-beta', '1.1.0-beta.1', -1],
    ['1.1.0-alpha.1', '1.1.0-beta.1', -1],
    ['1.1.0-1', '1.1.0-beta', -1],
    ['1.0.0+build.5', '1.0.0', 0],
  ])('%s vs %s → %i', (a, b, expected) => {
    expect(compareVersions(a, b)).toBe(expected);
    expect(compareVersions(b, a)).toBe(-expected || 0);
  });
});
