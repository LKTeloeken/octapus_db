import { create } from 'zustand';
import type { QueryColumnInfo } from '@/api/types/query.types';

/** Uma célula editada de uma linha existente */
export interface CellChange {
  columnId: string;
  originalValue: string | null;
  newValue: string | null;
}

/** Edições das linhas existentes: pkKey → coluna → alteração */
export type ChangesMap = Map<string, Map<string, CellChange>>;

/** A not-yet-persisted row; cells are aligned to `columns` by index */
export interface AddedRow {
  tempId: string;
  cells: (string | null)[];
  /** Índices de colunas editadas pelo usuário: distingue "não tocada" (o banco
   *  aplica o default) de NULL explícito */
  touched: Set<number>;
}

/**
 * As alterações pendentes de uma grade (`ResultsTable`). As regras moram em
 * `components/results-table/pending-changes.ts`; aqui fica só o estado.
 */
export interface PendingChanges {
  /** Colunas da grade em que as pendências nasceram (`columnsSignature`) */
  columnsSignature: string;
  changes: ChangesMap;
  /** Linhas novas, mostradas depois das existentes */
  addedRows: AddedRow[];
  /** pkKeys das linhas existentes marcadas para remoção (em vermelho) */
  removedRowKeys: ReadonlySet<string>;
}

export const NO_PENDING_CHANGES: PendingChanges = {
  columnsSignature: '',
  changes: new Map(),
  addedRows: [],
  removedRowKeys: new Set(),
};

/**
 * Identidade do conjunto de colunas (nome e tipo, na ordem). As linhas novas
 * são posicionais: pendências de outra grade não valem nesta.
 */
export const columnsSignature = (columns: QueryColumnInfo[]): string =>
  columns.map(c => `${c.name}\u0000${c.typeName}`).join('\u0001');

const isEmpty = (pending: PendingChanges) =>
  pending.changes.size === 0 &&
  pending.addedRows.length === 0 &&
  pending.removedRowKeys.size === 0;

/**
 * Alterações pendentes da grade, por aba (o id da aba é o escopo; o mesmo
 * `searchScopeId` da busca). Só a aba ativa fica montada, então o estado não
 * pode morar no componente: trocar de aba e voltar mantém as pendências.
 * Somem ao salvar, ao descartar, quando a grade muda de colunas (outra query)
 * e quando a aba fecha. Não entram na sessão persistida (`tabs-session.ts`):
 * são dados do banco, não da aba.
 */
interface PendingChangesState {
  byScope: Map<string, PendingChanges>;

  /**
   * Aplica `recipe` às pendências do escopo. Sem pendências depois disso, a
   * entrada some — o mapa só guarda grades com algo a salvar.
   */
  update: (
    scopeId: string,
    columnsSignature: string,
    recipe: (current: PendingChanges) => Partial<PendingChanges>,
  ) => void;
  clear: (scopeId: string) => void;
}

export const usePendingChangesStore = create<PendingChangesState>(set => ({
  byScope: new Map(),

  update: (scopeId, signature, recipe) =>
    set(state => {
      const current = state.byScope.get(scopeId) ?? {
        ...NO_PENDING_CHANGES,
        columnsSignature: signature,
      };
      const patch = recipe(current);
      const next = { ...current, ...patch, columnsSignature: signature };
      const unchanged = (Object.keys(patch) as (keyof PendingChanges)[]).every(
        key => patch[key] === current[key],
      );
      if (unchanged && state.byScope.has(scopeId)) return state;

      const byScope = new Map(state.byScope);
      if (isEmpty(next)) {
        if (!byScope.delete(scopeId)) return state;
      } else {
        byScope.set(scopeId, next);
      }
      return { byScope };
    }),

  clear: scopeId =>
    set(state => {
      if (!state.byScope.has(scopeId)) return state;
      const byScope = new Map(state.byScope);
      byScope.delete(scopeId);
      return { byScope };
    }),
}));

/** A aba tem algo a salvar? (fechar pede confirmação) */
export const hasPendingChanges = (scopeId: string): boolean =>
  usePendingChangesStore.getState().byScope.has(scopeId);
