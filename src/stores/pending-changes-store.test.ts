import { beforeEach, describe, expect, it } from 'vitest';
import type { QueryColumnInfo } from '@/api/types/query.types';
import {
  columnsSignature,
  hasPendingChanges,
  usePendingChangesStore,
} from './pending-changes-store';
import { useTabsStore } from './tabs-store';

const column = (name: string, typeName = 'text'): QueryColumnInfo => ({
  name,
  typeName,
  typeOid: null,
});

const SIGNATURE = columnsSignature([column('id', 'integer'), column('nome')]);

const removeRow = (scopeId: string, pkKey: string) =>
  usePendingChangesStore
    .getState()
    .update(scopeId, SIGNATURE, ({ removedRowKeys }) => ({
      removedRowKeys: new Set(removedRowKeys).add(pkKey),
    }));

describe('pending-changes-store', () => {
  beforeEach(() => {
    usePendingChangesStore.setState({ byScope: new Map() });
    useTabsStore.setState({ tabs: new Map(), activeTabId: null });
  });

  it('cada aba guarda as próprias pendências, com a assinatura das colunas', () => {
    removeRow('a', '["1"]');
    removeRow('b', '["2"]');

    const { byScope } = usePendingChangesStore.getState();
    expect(Array.from(byScope.get('a')!.removedRowKeys)).toEqual(['["1"]']);
    expect(Array.from(byScope.get('b')!.removedRowKeys)).toEqual(['["2"]']);
    expect(byScope.get('a')!.columnsSignature).toBe(SIGNATURE);
    expect(hasPendingChanges('a')).toBe(true);
    expect(hasPendingChanges('c')).toBe(false);
  });

  it('sem nada pendente a entrada some', () => {
    removeRow('a', '["1"]');
    usePendingChangesStore
      .getState()
      .update('a', SIGNATURE, () => ({ removedRowKeys: new Set() }));
    expect(usePendingChangesStore.getState().byScope.has('a')).toBe(false);
  });

  it('a assinatura distingue nome e tipo das colunas', () => {
    expect(columnsSignature([column('id', 'integer')])).not.toBe(
      columnsSignature([column('id', 'text')]),
    );
    expect(columnsSignature([column('a'), column('b')])).not.toBe(
      columnsSignature([column('b'), column('a')]),
    );
  });

  it('fechar a aba (ou as do servidor) descarta as pendências dela', () => {
    const tabs = useTabsStore.getState();
    const one = tabs.openQueryTab({ serverId: 1, database: 'loja' });
    const two = tabs.openQueryTab({ serverId: 1, database: 'loja' });
    const other = tabs.openQueryTab({ serverId: 2, database: 'main' });
    for (const id of [one, two, other]) removeRow(id, '["1"]');

    useTabsStore.getState().closeTab(one);
    expect(hasPendingChanges(one)).toBe(false);
    expect(hasPendingChanges(two)).toBe(true);

    useTabsStore.getState().closeServerTabs(1);
    expect(hasPendingChanges(two)).toBe(false);
    expect(hasPendingChanges(other)).toBe(true);
  });
});
