import { describe, expect, it } from 'vitest';
import type { PgToolsInstall, PgToolsStatus } from '@/api/types/backup.types';
import {
  DEFAULT_BACKUP_OPTIONS,
  backupTargetLabel,
  effectiveRestoreOptions,
  normalizeBackupOptions,
  restoreToolsProblem,
} from './backup-options';

describe('normalizeBackupOptions', () => {
  const plainAll = {
    ...DEFAULT_BACKUP_OPTIONS,
    format: 'plain' as const,
    clean: true,
    columnInserts: true,
  };

  it('mantém as opções do script SQL quando valem', () => {
    expect(normalizeBackupOptions(plainAll)).toEqual(plainAll);
  });

  it('desliga as opções só do SQL ao voltar para .dump', () => {
    const custom = normalizeBackupOptions({ ...plainAll, format: 'custom' });
    expect(custom.clean).toBe(false);
    expect(custom.columnInserts).toBe(false);
    expect(custom.noOwner).toBe(true);
  });

  it('cada conteúdo desliga a opção que briga com ele', () => {
    const dataOnly = normalizeBackupOptions({ ...plainAll, content: 'dataOnly' });
    expect(dataOnly.clean).toBe(false);
    expect(dataOnly.columnInserts).toBe(true);

    const schemaOnly = normalizeBackupOptions({ ...plainAll, content: 'schemaOnly' });
    expect(schemaOnly.clean).toBe(true);
    expect(schemaOnly.columnInserts).toBe(false);
  });
});

describe('backupTargetLabel', () => {
  it('descreve database, schema e tabela', () => {
    expect(backupTargetLabel('loja', { kind: 'database' })).toBe('loja');
    expect(backupTargetLabel('loja', { kind: 'schema', schema: 'vendas' })).toBe(
      'loja › vendas',
    );
    expect(
      backupTargetLabel('loja', { kind: 'table', schema: 'vendas', table: 'pedidos' }),
    ).toBe('loja › vendas.pedidos');
  });
});

describe('effectiveRestoreOptions', () => {
  it('no script SQL só vale o "tudo ou nada"', () => {
    const options = { noOwner: true, clean: true, singleTransaction: true };
    expect(effectiveRestoreOptions('plain', options)).toEqual({
      noOwner: false,
      clean: false,
      singleTransaction: true,
    });
    expect(effectiveRestoreOptions('custom', options)).toEqual(options);
  });
});

describe('restoreToolsProblem', () => {
  const install: PgToolsInstall = {
    dir: '/x',
    source: 'path',
    version: '15.8',
    versionNum: 150_008,
    pgDump: '/x/pg_dump',
    pgRestore: '/x/pg_restore',
    psql: null,
  };
  const status = (patch: Partial<PgToolsStatus> = {}): PgToolsStatus => ({
    selected: install,
    candidates: [install],
    configuredError: null,
    serverVersion: '16.4',
    serverVersionNum: 160_004,
    problem: null,
    ...patch,
  });

  it('cliente antigo não impede a restauração', () => {
    const old = status({
      problem: { kind: 'clientTooOld', client: '15.8', server: '16.4' },
    });
    expect(restoreToolsProblem('custom', old)).toBeNull();
  });

  it('cobra a ferramenta do formato', () => {
    expect(restoreToolsProblem('plain', status())).toMatch(/psql/);
    expect(restoreToolsProblem(null, status())).toBeNull();
  });

  it('sem pg_dump nenhum, explica', () => {
    expect(
      restoreToolsProblem('custom', status({ selected: null, problem: { kind: 'notFound' } })),
    ).toMatch(/não foi encontrado/);
  });
});
