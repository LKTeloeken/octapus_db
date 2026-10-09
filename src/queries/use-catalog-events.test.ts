import { describe, expect, it } from 'vitest';
import type { CatalogEvent } from '@/api/types/catalog.types';
import { catalogEventInvalidations } from './use-catalog-events';
import { queryKeys } from './keys';

const base = { serverId: 3, database: 'saas' };

describe('catalogEventInvalidations', () => {
  it('progresso sem conteúdo novo só toca o status', () => {
    for (const event of [
      { ...base, type: 'syncing' },
      { ...base, type: 'cancelled' },
      { ...base, type: 'error', message: 'boom' },
    ] satisfies CatalogEvent[]) {
      expect(catalogEventInvalidations(event)).toEqual({
        refetch: [queryKeys.catalogStatus(3, 'saas')],
        stale: [],
      });
    }
  });

  it('camadas novas invalidam o database e a busca da palette', () => {
    const plan = catalogEventInvalidations({
      ...base,
      type: 'schemas',
      added: ['t1'],
      removed: [],
    });
    expect(plan.refetch).toEqual([
      queryKeys.catalogScope(3, 'saas'),
      queryKeys.catalogSearchScope,
    ]);
    expect(plan.stale).toEqual([]);
  });

  it('schemas que mudaram deixam as colunas velhas, sem refetch', () => {
    const plan = catalogEventInvalidations({
      ...base,
      type: 'ready',
      added: [],
      removed: [],
      changed: ['tenant_00007', 'public'],
      fetchedAt: 1,
    });
    expect(plan.refetch).toContainEqual(queryKeys.catalogScope(3, 'saas'));
    expect(plan.stale).toEqual([
      queryKeys.columnsScope(3, 'saas', 'tenant_00007'),
      queryKeys.columnsScope(3, 'saas', 'public'),
    ]);
  });

  it('o escopo do database cobre status, filhos e tamanho', () => {
    const scope = queryKeys.catalogScope(3, 'saas');
    const covered = [
      queryKeys.catalogStatus(3, 'saas'),
      queryKeys.catalogChildren(3, 'saas', { kind: 'schemas' }, '', 500),
      queryKeys.catalogRelationSize(3, 'saas', 'public', 'plans'),
    ];
    for (const key of covered)
      expect(key.slice(0, scope.length)).toEqual([...scope]);
    // Outro database não entra
    expect(queryKeys.catalogStatus(3, 'other').slice(0, 3)).not.toEqual([
      ...scope,
    ]);
  });
});
