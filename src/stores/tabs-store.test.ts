import { beforeEach, describe, expect, it } from 'vitest';
import { useTabsStore } from './tabs-store';

const order = () =>
  Array.from(useTabsStore.getState().tabs.values(), t => t.title);

describe('moveTab (arrastar na barra de abas)', () => {
  beforeEach(() => {
    useTabsStore.setState({ tabs: new Map(), activeTabId: null });
    const { openQueryTab } = useTabsStore.getState();
    for (const title of ['a', 'b', 'c', 'd']) {
      openQueryTab({ serverId: 1, database: 'app', title });
    }
  });

  const idOf = (title: string) =>
    Array.from(useTabsStore.getState().tabs.values()).find(
      t => t.title === title,
    )!.id;

  it('leva a aba para a posição pedida, para frente e para trás', () => {
    useTabsStore.getState().moveTab(idOf('a'), 2);
    expect(order()).toEqual(['b', 'c', 'a', 'd']);
    useTabsStore.getState().moveTab(idOf('d'), 0);
    expect(order()).toEqual(['d', 'b', 'c', 'a']);
  });

  it('posição fora da faixa vai para a ponta; a aba ativa não muda', () => {
    const active = useTabsStore.getState().activeTabId;
    useTabsStore.getState().moveTab(idOf('b'), 99);
    expect(order()).toEqual(['a', 'c', 'd', 'b']);
    expect(useTabsStore.getState().activeTabId).toBe(active);
  });

  it('mesma posição ou aba inexistente não troca o estado', () => {
    const before = useTabsStore.getState().tabs;
    useTabsStore.getState().moveTab(idOf('c'), 2);
    useTabsStore.getState().moveTab('nao-existe', 0);
    expect(useTabsStore.getState().tabs).toBe(before);
  });
});
