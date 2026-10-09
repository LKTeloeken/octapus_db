import { describe, expect, it } from 'vitest';
import type { CatalogDiagnostics } from '@/api/types/catalog.types';
import { formatCatalogDiagnostics } from './catalog-diagnostics';

const diagnostics = (
  overrides: Partial<CatalogDiagnostics> = {},
): CatalogDiagnostics => ({
  appVersion: '0.1.0-beta.17',
  status: {
    serverId: 1,
    database: 'cliente_secreto',
    syncing: false,
    fetchedAt: Date.UTC(2026, 9, 9, 12, 0, 0),
    fromDisk: false,
    error: null,
    serverVersion: 'PostgreSQL 16.4',
    stats: {
      schemas: 5002,
      loaded: 5002,
      stale: 0,
      unloaded: 0,
      shapes: 4,
      distinctNames: 454,
      relations: 750_154,
      approxHeapBytes: 430_000,
    },
  },
  lastSync: {
    at: Date.UTC(2026, 9, 9, 12, 0, 0),
    strategy: 'shapeFirst',
    schemas: 5002,
    shapes: 6,
    fetched: 6,
    shared: 4996,
    added: 5002,
    removed: 0,
    changed: 0,
    layer0Ms: 135.4,
    totalMs: 1980,
  },
  drift: {
    dominantTables: 150,
    dominantSchemas: 4950,
    divergentGroups: 1,
    divergentSchemas: 50,
  },
  ...overrides,
});

describe('formatCatalogDiagnostics', () => {
  it('traz contagens e tempos, sem o nome do database', () => {
    const text = formatCatalogDiagnostics(diagnostics(), 'postgres');
    expect(text).not.toContain('cliente_secreto');
    expect(text.split('\n')).toEqual([
      'Octapus 0.1.0-beta.17 · diagnóstico do catálogo',
      'Banco: PostgreSQL (PostgreSQL 16.4)',
      'Estado: pronto',
      'Última revalidação: 2026-10-09T12:00:00.000Z',
      'Schemas: 5.002 (carregados 5.002, desatualizados 0, pendentes 0)',
      expect.stringMatching(
        /^Formatos: 4 · nomes distintos: 454 · relações: 750\.154 · memória: ~/,
      ),
      'Última sincronização (2026-10-09T12:00:00.000Z): por formato (6 no servidor); 6 schemas lidos, 4.996 copiados do formato; camada 0 em 135 ms, total 1,98 s; +5.002 −0 ~0',
      'Drift: formato principal com 150 relações em 4.950 schemas; 1 variação em 50 schemas',
    ]);
  });

  it('erro, conteúdo do disco e nenhuma sincronização ainda', () => {
    const base = diagnostics();
    const text = formatCatalogDiagnostics(
      diagnostics({
        status: {
          ...base.status,
          error: 'timeout',
          fromDisk: true,
          fetchedAt: null,
        },
        lastSync: null,
        drift: {
          dominantTables: null,
          dominantSchemas: 0,
          divergentGroups: 0,
          divergentSchemas: 0,
        },
      }),
      'mongodb',
    );
    expect(text).toContain('Banco: MongoDB');
    expect(text).toContain(
      'Estado: erro: timeout · conteúdo do disco, ainda não revalidado',
    );
    expect(text).toContain('Última revalidação: nunca');
    expect(text).toContain('Última sincronização: nenhuma nesta sessão');
    expect(text).toContain('Drift: sem formato repetido');
  });
});
