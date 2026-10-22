import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CatalogObjectRef } from '@/api/types/catalog.types';

const session = vi.hoisted(() => ({ saved: null as string | null }));
vi.mock('@/api/session', () => ({
  loadSession: () => Promise.resolve(session.saved),
  saveSession: () => Promise.resolve(),
}));

import { restoreTabsSession } from './tabs-session';
import { useTabsStore } from './tabs-store';

const target: CatalogObjectRef = {
  kind: 'function',
  schema: 'vendas',
  name: 'total',
  table: null,
  signature: 'integer, date',
};

const open = (overrides: Partial<CatalogObjectRef> = {}) =>
  useTabsStore.getState().openDefinitionTab({
    serverId: 1,
    database: 'loja',
    schema: 'vendas',
    target: { ...target, ...overrides },
    title: 'total(integer, date)',
  });

beforeEach(() => useTabsStore.setState({ tabs: new Map(), activeTabId: null }));

describe('aba de definição', () => {
  it('uma aba por objeto: abrir de novo só ativa', () => {
    const first = open();
    useTabsStore.getState().openQueryTab({ serverId: 1, database: 'loja' });
    expect(open()).toBe(first);
    expect(useTabsStore.getState().activeTabId).toBe(first);
    // Outra sobrecarga é outro objeto
    expect(open({ signature: 'integer' })).not.toBe(first);
    expect(useTabsStore.getState().tabs.size).toBe(3);
  });

  it('volta da sessão com o mesmo id (e descarta objeto inválido)', async () => {
    const id = open();
    const tab = useTabsStore.getState().tabs.get(id);
    session.saved = JSON.stringify({
      version: 1,
      activeTabId: id,
      tabs: [
        tab,
        { ...tab, id: 'x', target: { kind: 'tabela', schema: 's', name: 'n' } },
      ],
    });
    useTabsStore.setState({ tabs: new Map(), activeTabId: null });

    await restoreTabsSession();
    const { tabs, activeTabId } = useTabsStore.getState();
    expect(Array.from(tabs.keys())).toEqual([id]);
    expect(activeTabId).toBe(id);
    expect(tabs.get(id)).toEqual(tab);
  });
});
