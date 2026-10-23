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

export interface SeedQueryTab {
  id: string;
  title: string;
  content: string;
}

export interface SeedBrowseTab {
  id: string;
  title: string;
  table: string;
  schema?: string;
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
            ...PG_SERVER,
            id: tab.id,
            kind: 'query',
            title: tab.title,
            schema: null,
            content: tab.content,
            hiddenColumns: [],
          }
        : {
            ...PG_SERVER,
            id: tab.id,
            kind: 'browse',
            title: tab.title,
            schema: tab.schema ?? 'public',
            table: tab.table,
            whereExpr: '',
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
  { tabs = [], active }: { tabs?: SeedTab[]; active?: string } = {},
) {
  const session = tabs.length
    ? sessionSnapshot(tabs, active ?? tabs[0].id)
    : null;

  await page.addInitScript(session => {
    if (sessionStorage.getItem('e2e-seeded')) return;
    sessionStorage.setItem('e2e-seeded', '1');
    // Sem latência artificial: a espera fica por conta do auto-wait do Playwright
    localStorage.setItem(
      'octapus-mock',
      JSON.stringify({ state: { latencyMs: 0 }, version: 0 }),
    );
    if (session) localStorage.setItem('octapus-mock-session', session);
  }, session);

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
