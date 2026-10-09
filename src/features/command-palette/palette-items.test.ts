import { describe, expect, it } from 'vitest';
import { FLAT_SCHEMA, type CatalogSearchHit } from '@/api/types/catalog.types';
import { encodeNodeId } from '@/lib/node-ref';
import {
  catalogHitToItem,
  flattenGroups,
  groupByServer,
  inPinnedSchema,
  pinnedTableItem,
  resolvePin,
  tableEntry,
} from './palette-items';

const server = { id: 1, name: 'SaaS', dbType: 'postgres' as const };
const hit = (overrides: Partial<CatalogSearchHit>): CatalogSearchHit => ({
  serverId: 1,
  database: 'saas',
  name: 'orders',
  kind: 'table',
  score: 100,
  schema: null,
  schemas: null,
  ...overrides,
});

describe('catalogHitToItem', () => {
  it('a mesma tabela em vários schemas vira um grupo', () => {
    const item = catalogHitToItem(
      hit({
        schemas: { total: 4950, sample: ['tenant_00001', 'tenant_00002'] },
      }),
      'ord',
      server,
    )!;
    expect(item.target.kind).toBe('group');
    expect(item.label).toBe('orders');
    expect(item.subtitle).toBe('em 4.950 schemas · saas');
    expect(item.indices).toEqual([0, 1, 2]);
  });

  it('um schema só ou um par schema.tabela abre direto', () => {
    const single = catalogHitToItem(
      hit({ schemas: { total: 1, sample: ['public'] } }),
      'ord',
      server,
    )!;
    const pair = catalogHitToItem(
      hit({ schema: 'tenant_00042' }),
      'tenant_42.ord',
      server,
    )!;

    for (const [item, schema] of [
      [single, 'public'],
      [pair, 'tenant_00042'],
    ] as const) {
      expect(item.target.kind).toBe('table');
      expect(item.key).toBe(
        encodeNodeId({
          serverId: 1,
          database: 'saas',
          schema,
          table: 'orders',
        }),
      );
      expect(item.label).toBe(`${schema}.orders`);
    }
    expect(pair.indices.length).toBeGreaterThan(0);
  });

  it('um schema lista as relações dele', () => {
    const item = catalogHitToItem(
      hit({ name: 'tenant_00042', kind: 'schema' }),
      'tenant',
      server,
    )!;
    expect(item.target).toMatchObject({
      kind: 'schema',
      schema: 'tenant_00042',
    });
    expect(item.subtitle).toBe('schema · saas');
  });

  it('sem schema (Mongo, SQLite): abre a coleção, e a aba fica sem schema', () => {
    const mongo = { id: 2, name: 'Mongo', dbType: 'mongodb' as const };
    const item = catalogHitToItem(
      hit({
        serverId: 2,
        database: 'app',
        schemas: { total: 1, sample: [FLAT_SCHEMA] },
      }),
      'ord',
      mongo,
    )!;
    expect(item.target.kind).toBe('table');
    expect(item.key).toBe(
      encodeNodeId({ serverId: 2, database: 'app', table: 'orders' }),
    );
    expect(item.label).toBe('orders');
    expect(item.target.kind === 'table' && item.target.entry.schema).toBeNull();
  });

  it('servidor desconhecido não vira item', () => {
    expect(catalogHitToItem(hit({}), 'ord', undefined)).toBeNull();
  });
});

describe('groupByServer + flattenGroups', () => {
  it('agrupa por servidor na ordem de chegada e intercala os títulos', () => {
    const other = { id: 2, name: 'Outro', dbType: 'postgres' as const };
    const items = [
      catalogHitToItem(hit({ schema: 'a' }), 'o', server)!,
      catalogHitToItem(hit({ serverId: 2, schema: 'b' }), 'o', other)!,
      catalogHitToItem(hit({ schema: 'c' }), 'o', server)!,
    ];
    const groups = groupByServer(items);
    expect(groups.map(group => [group.heading, group.items.length])).toEqual([
      ['SaaS', 2],
      ['Outro', 1],
    ]);
    expect(flattenGroups(groups).map(row => row.kind)).toEqual([
      'header',
      'item',
      'item',
      'header',
      'item',
    ]);
  });
});

describe('schema fixado', () => {
  const schemaItem = catalogHitToItem(
    hit({ name: 'tenant_00042', kind: 'schema' }),
    'tnt42',
    server,
  );
  const tableItem = catalogHitToItem(hit({ schema: 'public' }), 'ord', server);

  it('o Tab fixa o schema do item ativo, se ele for um schema', () => {
    expect(resolvePin('tnt42', schemaItem)).toEqual({
      schema: 'tenant_00042',
      rest: '',
    });
  });

  it('senão, o trecho antes do ponto — e o resto segue na busca', () => {
    expect(resolvePin('public.us', tableItem)).toEqual({
      schema: 'public',
      rest: 'us',
    });
    expect(resolvePin(' public ', tableItem)).toEqual({
      schema: 'public',
      rest: '',
    });
  });

  it('sem texto (ou só `.tabela`) não há o que fixar', () => {
    expect(resolvePin('', null)).toBeNull();
    expect(resolvePin('   ', tableItem)).toBeNull();
    expect(resolvePin('.orders', null)).toBeNull();
  });

  it('o item mostra só a tabela, com o schema na legenda', () => {
    const entry = tableEntry(server, 'saas', 'tenant_00042', 'orders');
    const item = pinnedTableItem(entry, 'ord');
    expect(item.label).toBe('orders');
    expect(item.indices).toEqual([0, 1, 2]);
    expect(item.subtitle).toBe('tenant_00042 · saas');
    expect(item.key).toBe(entry.id);
  });

  it('o filtro local ignora maiúsculas e entradas sem schema', () => {
    expect(
      inPinnedSchema(tableEntry(server, 'saas', 'Public', 'a'), 'public'),
    ).toBe(true);
    expect(
      inPinnedSchema(tableEntry(server, 'saas', 'other', 'a'), 'public'),
    ).toBe(false);
    expect(
      inPinnedSchema(tableEntry(server, 'saas', null, 'a'), 'public'),
    ).toBe(false);
  });
});
