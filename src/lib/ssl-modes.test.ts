import { describe, expect, it } from 'vitest';
import { sslModeFor, sslModesFor, sslModeVerifies } from './ssl-modes';

describe('ssl-modes', () => {
  it('cada banco oferece só o que aceita', () => {
    expect(sslModesFor('postgres').map(o => o.value)).toEqual([
      'disable',
      'prefer',
      'require',
      'verify-ca',
      'verify-full',
    ]);
    expect(sslModesFor('mongodb').map(o => o.value)).toEqual([
      'disable',
      'require',
      'verify-full',
    ]);
    expect(sslModesFor('sqlite')).toEqual([]);
  });

  it('ajusta o modo ao trocar de banco, como o backend', () => {
    expect(sslModeFor('verify-ca', 'postgres')).toBe('verify-ca');
    expect(sslModeFor('verify-ca', 'redis')).toBe('verify-full');
    expect(sslModeFor('prefer', 'mongodb')).toBe('disable');
    expect(sslModeFor('require', 'sqlite')).toBe('disable');
  });

  it('só os verify-* conferem o certificado', () => {
    expect(sslModeVerifies('verify-full')).toBe(true);
    expect(sslModeVerifies('verify-ca')).toBe(true);
    expect(sslModeVerifies('require')).toBe(false);
    expect(sslModeVerifies(null)).toBe(false);
  });
});
