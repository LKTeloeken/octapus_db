import { describe, expect, it } from 'vitest';
import type { PgToolsInstall } from '@/api/types/backup.types';
import { describePgToolsProblem, missingPgTools } from './pg-tools';

const install = (patch: Partial<PgToolsInstall> = {}): PgToolsInstall => ({
  dir: '/opt/homebrew/bin',
  source: 'path',
  version: '16.4',
  versionNum: 160_004,
  pgDump: '/opt/homebrew/bin/pg_dump',
  pgRestore: '/opt/homebrew/bin/pg_restore',
  psql: '/opt/homebrew/bin/psql',
  ...patch,
});

describe('describePgToolsProblem', () => {
  it('pede a major do servidor quando o cliente é antigo', () => {
    const text = describePgToolsProblem({
      kind: 'clientTooOld',
      client: '15.8',
      server: '16.4',
    });
    expect(text).toContain('pg_dump 15.8');
    expect(text).toContain('versão 16 ou mais nova');
  });

  it('repete a mensagem da pasta configurada', () => {
    const text = describePgToolsProblem({
      kind: 'invalidConfigured',
      message: 'Não há pg_dump em /x.',
    });
    expect(text.startsWith('Não há pg_dump em /x.')).toBe(true);
  });

  it('explica como instalar quando não acha nada', () => {
    expect(describePgToolsProblem({ kind: 'notFound' })).toMatch(
      /^O pg_dump não foi encontrado\. Instale/,
    );
  });
});

describe('missingPgTools', () => {
  it('lista só o que falta', () => {
    expect(missingPgTools(install())).toEqual([]);
    expect(missingPgTools(install({ pgRestore: null, psql: null }))).toEqual([
      'pg_restore',
      'psql',
    ]);
  });
});
