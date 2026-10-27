import {
  test as base,
  expect,
  type Locator,
  type Page,
} from '@playwright/test';

/**
 * Base dos testes e2e. Cada teste roda num contexto de navegador novo — sem
 * `localStorage` nem IndexedDB — e com o dataset do mock recém-gerado, então
 * nada vaza de um teste para outro.
 */

/** Servidor Postgres do dataset do mock ("Loja (local)") */
export const PG_SERVER = { serverId: 1, database: 'loja' } as const;

/** Servidor e banco da aba; sem eles, o Postgres `loja` (`PG_SERVER`) */
interface SeedTarget {
  serverId?: number;
  database?: string;
}

export interface SeedQueryTab extends SeedTarget {
  id: string;
  title: string;
  content: string;
}

export interface SeedBrowseTab extends SeedTarget {
  id: string;
  title: string;
  table: string;
  schema?: string;
  whereExpr?: string;
}

export type SeedTab =
  | ({ kind: 'query' } & SeedQueryTab)
  | ({ kind: 'browse' } & SeedBrowseTab);

/**
 * Monta o snapshot que o `restoreTabsSession` lê no boot (src/stores/tabs-session.ts).
 * No mock a sessão mora no `localStorage`, então o app já abre com essas abas.
 */
function sessionSnapshot(tabs: SeedTab[], activeTabId: string): string {
  return JSON.stringify({
    version: 1,
    activeTabId,
    tabs: tabs.map(tab =>
      tab.kind === 'query'
        ? {
            serverId: tab.serverId ?? PG_SERVER.serverId,
            database: tab.database ?? PG_SERVER.database,
            id: tab.id,
            kind: 'query',
            title: tab.title,
            schema: null,
            content: tab.content,
            hiddenColumns: [],
          }
        : {
            serverId: tab.serverId ?? PG_SERVER.serverId,
            database: tab.database ?? PG_SERVER.database,
            id: tab.id,
            kind: 'browse',
            title: tab.title,
            schema: tab.schema ?? 'public',
            table: tab.table,
            whereExpr: tab.whereExpr ?? '',
            sort: [],
            hiddenColumns: [],
          },
    ),
  });
}

/**
 * Abre o app com as abas dadas. A semente só entra no primeiro carregamento da
 * página: depois de um `reload()` vale a sessão que o próprio app gravou.
 */
export async function openApp(
  page: Page,
  {
    tabs = [],
    active,
    assistant = false,
  }: {
    tabs?: SeedTab[];
    active?: string;
    /** Já entrou no assistente e aceitou o termo do mock */
    assistant?: boolean;
  } = {},
) {
  const session = tabs.length
    ? sessionSnapshot(tabs, active ?? tabs[0].id)
    : null;

  await page.addInitScript(
    ({ session, assistant }) => {
      if (sessionStorage.getItem('e2e-seeded')) return;
      sessionStorage.setItem('e2e-seeded', '1');
      // Sem latência artificial: a espera fica por conta do auto-wait do Playwright
      localStorage.setItem(
        'octapus-mock',
        JSON.stringify({ state: { latencyMs: 0 }, version: 0 }),
      );
      if (session) localStorage.setItem('octapus-mock-session', session);
      // A versão é a do termo do mock (src/mocks/assistant.ts)
      if (assistant) {
        localStorage.setItem(
          'octapus-mock-assistant',
          JSON.stringify({ loggedIn: true, acceptedVersion: '2026-10-v1' }),
        );
      }
    },
    { session, assistant },
  );

  await page.goto('/');
  await expect(page.locator('#root')).not.toBeEmpty();
}

/** Aba do documento (linha de abas do topo) pelo título */
export const workspaceTab = (page: Page, title: string): Locator =>
  page
    .locator('[data-tauri-drag-region] > [role=tablist]')
    .getByRole('tab', { name: title });

export const activeWorkspaceTab = (page: Page): Locator =>
  page.locator(
    '[data-tauri-drag-region] > [role=tablist] [role=tab][aria-selected=true]',
  );

/** Atalho com a tecla de comando da plataforma (Cmd no macOS, Ctrl fora dele) */
export const mod = (key: string) => `ControlOrMeta+${key}`;

/**
 * Grade de resultados (`ResultsTable`) da aba ativa. Linhas e colunas pelos
 * atributos ARIA da grade: `aria-rowindex` (base 1, o cabeçalho é a linha 1) e
 * o nome da coluna em `data-column`. A grade é virtualizada: só as linhas e
 * colunas visíveis existem no DOM.
 */
export function grid(page: Page) {
  const root = page.getByRole('grid');
  const row = (index: number): Locator =>
    root.locator(`[role=row][aria-rowindex="${index + 2}"]`);
  const cell = (index: number, column: string): Locator =>
    row(index).locator(`[role=gridcell][data-column="${column}"]`);
  /** O botão da célula fechada, cujo texto é o valor exibido */
  const value = (index: number, column: string): Locator =>
    cell(index, column).getByRole('button').first();
  /** Editor de texto aberto pelo duplo clique (popover) */
  const editor = page.getByRole('dialog').getByRole('textbox');
  /** Barra flutuante de alterações pendentes */
  const pendingBar = page.getByRole('status').filter({ hasText: /alteraç/ });

  return {
    root,
    row,
    cell,
    value,
    editor,
    pendingBar,
    /** Contagem total da barra ("3 alterações"); a barra some com zero */
    pendingTotal: pendingBar.locator('span').first(),
    /** Abre o editor da célula com duplo clique */
    openEditor: async (index: number, column: string) => {
      await value(index, column).dblclick();
      await expect(editor).toBeFocused();
    },
    /** Edita a célula pelo editor de texto e grava com Cmd/Ctrl+Enter */
    edit: async (index: number, column: string, text: string) => {
      await value(index, column).dblclick();
      await expect(editor).toBeFocused();
      await editor.fill(text);
      await editor.press(mod('Enter'));
      await expect(editor).toBeHidden();
    },
    /** Seleciona a linha pelo gutter (o número da linha) */
    selectRow: (index: number, modifiers?: ('Shift' | 'ControlOrMeta')[]) =>
      row(index).getByRole('button').first().click({ modifiers }),
    save: () => pendingBar.getByRole('button', { name: /Salvar/ }).click(),
    discard: () =>
      pendingBar.getByRole('button', { name: 'Descartar' }).click(),
  };
}

export type Grid = ReturnType<typeof grid>;

/**
 * Abre um nó da árvore de conexões pelo caminho de rótulos (servidor, banco,
 * schema, pasta, tabela), clicando um por um. Os nós do caminho precisam estar
 * fechados: clicar num nó aberto o fecha.
 */
export async function openFromTree(page: Page, path: string[]) {
  const tree = page.getByRole('tree');
  for (const label of path) {
    await tree.getByText(label, { exact: true }).click();
  }
}

/** Cabeçalho da coluna na grade (some quando a coluna está oculta) */
export const columnHeader = (page: Page, column: string): Locator =>
  page
    .getByRole('grid')
    .locator(`[role=columnheader][data-column="${column}"]`);

/** A próxima chamada ao backend simulado falha (`setFailMode('next')`) */
export const failNextCommand = (page: Page): Promise<void> =>
  page.evaluate(() => {
    type MockControl = {
      store: { getState(): { setFailMode(mode: 'next'): void } };
    };
    const mock = (window as unknown as { __octapusMock: MockControl })
      .__octapusMock;
    mock.store.getState().setFailMode('next');
  });

export interface SessionTab {
  id: string;
  kind: string;
  title: string;
  serverId: number;
  database: string;
  [key: string]: unknown;
}

/**
 * A sessão de abas que o app gravou (no mock, no `localStorage`). A gravação
 * tem throttle: use com `expect.poll` antes de um `reload()`.
 */
export const savedSession = (
  page: Page,
): Promise<{ activeTabId: string | null; tabs: SessionTab[] } | null> =>
  page.evaluate(() => {
    const raw = localStorage.getItem('octapus-mock-session');
    return raw ? JSON.parse(raw) : null;
  });

/** Confirmação de fechar uma aba com alterações pendentes */
export const closeTabDialog = (page: Page): Locator =>
  page.getByRole('alertdialog', { name: /Descartar as alterações pendentes/ });

/**
 * Confirmação antes de recarregar a grade com alterações pendentes (ordenar,
 * filtrar, recarregar, reexecutar a query): Salvar, Descartar ou Cancelar
 */
export const pendingChangesDialog = (page: Page): Locator =>
  page.getByRole('alertdialog', { name: /Salvar as alterações pendentes/ });

export interface EditorSnapshot {
  doc: string;
  selection: { anchor: number; head: number }[];
  scrollTop: number;
}

/**
 * Estado do CodeMirror visível na tela, lido do `EditorView` pelo DOM. Na 6.41
 * a view fica em `cmTile.view`; versões antigas usavam `cmView.view`.
 */
export function readEditor(page: Page): Promise<EditorSnapshot | null> {
  return page.evaluate(() => {
    type View = {
      state: {
        doc: { toString(): string };
        selection: { ranges: readonly { anchor: number; head: number }[] };
      };
      scrollDOM: HTMLElement;
    };
    const content = document.querySelector('.cm-content') as
      | (HTMLElement & { cmTile?: { view: View }; cmView?: { view: View } })
      | null;
    const view = content?.cmTile?.view ?? content?.cmView?.view;
    if (!view) return null;
    return {
      doc: view.state.doc.toString(),
      selection: view.state.selection.ranges.map(({ anchor, head }) => ({
        anchor,
        head,
      })),
      scrollTop: view.scrollDOM.scrollTop,
    };
  });
}

/**
 * Grava o texto do editor no primeiro quadro depois do próximo clique — para
 * pegar piscadas que o auto-wait do Playwright esconderia. Ler com
 * `firstFrameText`.
 */
export async function captureFirstFrameAfterClick(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { __e2eFirstFrame?: Promise<string> };
    w.__e2eFirstFrame = new Promise(resolve => {
      document.addEventListener(
        'click',
        () =>
          requestAnimationFrame(() =>
            resolve(document.querySelector('.cm-content')?.textContent ?? ''),
          ),
        { capture: true, once: true },
      );
    });
  });
}

export const firstFrameText = (page: Page): Promise<string> =>
  page.evaluate(
    () =>
      (window as unknown as { __e2eFirstFrame?: Promise<string> })
        .__e2eFirstFrame ?? Promise.resolve(''),
  );

export { base as test, expect };
