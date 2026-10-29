import {
  activeWorkspaceTab,
  expect,
  openApp,
  savedSession,
  test,
  workspaceTab,
  type SeedTab,
} from './fixtures';
import type { Page } from '@playwright/test';

/**
 * A barra de abas: reordenar arrastando e o rótulo que separa abas de mesmo
 * título (a mesma tabela em outro schema, banco ou servidor).
 */

/** Títulos curtos: as três cabem até na menor janela, sem rolar a barra */
const query = (id: string): SeedTab => ({
  kind: 'query',
  id,
  title: id,
  content: `-- ${id}`,
});
const [A, B, C] = ['A', 'B', 'C'].map(query);

const events = (database: string, schema: string, id: string): SeedTab => ({
  kind: 'browse',
  id,
  title: 'events',
  database,
  schema,
  table: 'events',
});

/** Títulos das abas na ordem da barra */
const tabOrder = (page: Page) =>
  page
    .locator('[data-tab-id] [role=tab]')
    .evaluateAll(tabs => tabs.map(tab => tab.textContent));

/** Arrasta a aba `from` até a borda esquerda (ou direita) da aba `to` */
async function dragTab(
  page: Page,
  from: string,
  to: string,
  side: 'left' | 'right',
) {
  const source = await workspaceTab(page, from).boundingBox();
  const target = await workspaceTab(page, to).boundingBox();
  if (!source || !target) throw new Error('aba fora da tela');

  await page.mouse.move(
    source.x + source.width / 2,
    source.y + source.height / 2,
  );
  await page.mouse.down();
  const x = side === 'left' ? target.x + 4 : target.x + target.width - 4;
  await page.mouse.move(x, target.y + target.height / 2, { steps: 8 });
  await page.mouse.up();
}

test.describe('reordenar abas', () => {
  test('1. arrastar leva a aba para outra posição e a ordem volta no reload', async ({
    page,
  }) => {
    await openApp(page, { tabs: [A, B, C], active: 'A' });
    await expect.poll(() => tabOrder(page)).toEqual(['A', 'B', 'C']);

    await dragTab(page, 'C', 'A', 'left');
    await expect.poll(() => tabOrder(page)).toEqual(['C', 'A', 'B']);
    // Soltar não é clique: a aba arrastada fica ativa (o mousedown a ativou)
    await expect(activeWorkspaceTab(page)).toHaveText('C');

    await dragTab(page, 'C', 'B', 'right');
    await expect.poll(() => tabOrder(page)).toEqual(['A', 'B', 'C']);

    await dragTab(page, 'A', 'B', 'right');
    await expect.poll(() => tabOrder(page)).toEqual(['B', 'A', 'C']);

    // A ordem é a da sessão
    await expect
      .poll(async () => (await savedSession(page))?.tabs.map(t => t.title))
      .toEqual(['B', 'A', 'C']);
    await page.reload();
    await expect.poll(() => tabOrder(page)).toEqual(['B', 'A', 'C']);
  });

  test('2. clique sem arrastar só troca de aba', async ({ page }) => {
    await openApp(page, { tabs: [A, B, C], active: 'A' });

    await workspaceTab(page, 'B').click();
    await expect(activeWorkspaceTab(page)).toHaveText('B');
    await expect.poll(() => tabOrder(page)).toEqual(['A', 'B', 'C']);
  });

  test('3. o "x" da aba continua fechando, sem arrastar', async ({ page }) => {
    await openApp(page, { tabs: [A, B, C], active: 'A' });

    await page.getByRole('button', { name: 'Fechar aba B' }).click();
    await expect.poll(() => tabOrder(page)).toEqual(['A', 'C']);
  });
});

test.describe('abas de mesmo título', () => {
  test('4. ganham o que as distingue; as outras ficam como estão', async ({
    page,
  }) => {
    await openApp(page, {
      tabs: [
        events('saas', 'tenant_00001', 'e1'),
        events('saas', 'tenant_00002', 'e2'),
        { kind: 'browse', id: 'u', title: 'usuarios', table: 'usuarios' },
      ],
    });

    // Mesmo banco, schemas diferentes: só o schema
    await expect(workspaceTab(page, 'events em tenant_00001')).toBeVisible();
    await expect(workspaceTab(page, 'events em tenant_00002')).toBeVisible();
    await expect(workspaceTab(page, 'usuarios')).toHaveText('usuarios');
    // O caminho inteiro fica no title de toda aba
    // (o id da aba de tabela é refeito ao restaurar a sessão: acha pelo título)
    const usuarios = page
      .locator('[data-tab-id]')
      .filter({ has: page.getByRole('tab', { name: 'usuarios' }) });
    await expect(usuarios).toHaveAttribute(
      'title',
      'Loja (local) › loja › public',
    );

    // Fechou a gêmea: o rótulo some
    await page
      .getByRole('button', { name: 'Fechar aba events' })
      .first()
      .click();
    await expect(page.locator('[data-tab-id] [role=tab]')).toHaveText([
      'events',
      'usuarios',
    ]);
  });
});
