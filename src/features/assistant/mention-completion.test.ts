import { beforeEach, describe, expect, it, vi } from 'vitest';
import { catalogComplete } from '@/api/catalog';
import { getDataset } from '@/mocks/data';
import { SAAS_DATABASE } from '@/mocks/data/tenants';
import { useMockStore } from '@/mocks/mock-store';
import { route } from '@/mocks/router';
import { mentionOptions, type CompleteFetch } from './mention-completion';

// O `invoke` do app passa pelo router do mock, como no `pnpm dev:mock`
const calls: string[] = [];
vi.mock('@tauri-apps/api/core', async original => ({
  ...(await original<typeof import('@tauri-apps/api/core')>()),
  invoke: (command: string, args?: Record<string, unknown>) => {
    calls.push(command);
    return route(command, args);
  },
}));

const serverOf = (dbType: string) => {
  const entry = getDataset().servers.find(
    item => item.server.dbType === dbType,
  );
  if (!entry) throw new Error(`sem servidor ${dbType} no mock`);
  return entry;
};

const fetchFor =
  (serverId: number, database: string): CompleteFetch =>
  (schema, prefix) =>
    catalogComplete(serverId, database, schema, prefix, 50);

const labels = (found: Awaited<ReturnType<typeof mentionOptions>>) =>
  found?.options.map(option => option.label) ?? [];

describe('menções com @ sobre o catálogo do mock', () => {
  beforeEach(() => {
    useMockStore.setState({ latencyMs: 0, failMode: 'off' });
    calls.length = 0;
  });

  it('no saas (5.000 tenants) pede só fatias por prefixo', async () => {
    const { server } = serverOf('postgres');
    const fetch = fetchFor(server.id, SAAS_DATABASE);

    const atSign = await mentionOptions('liste @', {
      fetch,
      flat: false,
      defaultSchema: null,
    });
    // Schemas limitados a uma página, mais as tabelas do schema padrão
    expect(atSign?.options.length).toBeLessThanOrEqual(100);

    const tenants = await mentionOptions('liste @tenant_0004', {
      fetch,
      flat: false,
      defaultSchema: null,
    });
    const schemaLabels = labels(tenants).filter(label => label.endsWith('.'));
    expect(schemaLabels.length).toBeGreaterThan(0);
    expect(schemaLabels.length).toBeLessThanOrEqual(50);
    expect(schemaLabels.every(label => label.startsWith('@tenant_0004'))).toBe(
      true,
    );

    const relations = await mentionOptions('pedidos de @tenant_00042.ord', {
      fetch,
      flat: false,
      defaultSchema: null,
    });
    expect(labels(relations)).toContain('@tenant_00042.orders');
    expect(tenants?.text).toBe('@tenant_0004');

    // Nada de estrutura inteira: só o catalog_complete
    expect(new Set(calls)).toEqual(new Set(['catalog_complete']));
  });

  it('no SQLite, @tabela direto', async () => {
    const { server, databases } = serverOf('sqlite');
    const database = databases[0].name;
    const found = await mentionOptions('notas de @us', {
      fetch: fetchFor(server.id, database),
      flat: true,
      defaultSchema: null,
    });
    expect(labels(found)).toEqual(['@usuarios']);
    // `@a.b` não existe sem schema
    expect(
      await mentionOptions('@a.b', {
        fetch: fetchFor(server.id, database),
        flat: true,
        defaultSchema: null,
      }),
    ).toBeNull();
  });

  it('fora de uma menção não pede nada', async () => {
    const fetch = vi.fn<CompleteFetch>(async () => []);
    expect(
      await mentionOptions('mande para ana@empresa', {
        fetch,
        flat: false,
        defaultSchema: null,
      }),
    ).toBeNull();
    expect(fetch).not.toHaveBeenCalled();
  });
});
