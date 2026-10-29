import {
  expect,
  grid,
  mod,
  openApp,
  test,
  workspaceTab,
  type Grid,
  type SeedTab,
} from './fixtures';
import type { Page } from '@playwright/test';

/**
 * Duplicar, selecionar blocos de células e copiar/colar na grade. A área de
 * transferência do navegador pede gesto e permissão (e o WebKit não deixa ler):
 * aqui ela vira um texto em memória, lido e escrito pelo mesmo
 * `navigator.clipboard` que o mock do plugin do Tauri usa.
 */

/** Poucas linhas, e a linha salva (id novo, > 5000) volta pelo filtro */
const USUARIOS: SeedTab = {
  kind: 'browse',
  id: 'browse-usuarios',
  title: 'usuarios',
  table: 'usuarios',
  whereExpr: 'id <= 3 OR id > 5000',
};

/** A mesma tabela em dois tenants (mesmas colunas: id, tenant_ref, name, status) */
const tenantEvents = (tenant: string): SeedTab => ({
  kind: 'browse',
  id: `browse-events-${tenant}`,
  title: 'events',
  database: 'saas',
  schema: tenant,
  table: 'events',
  whereExpr: 'id <= 3',
});
const EVENTS_1 = tenantEvents('tenant_00001');
const EVENTS_2 = tenantEvents('tenant_00002');
const EVENTS_COLUMNS = ['id', 'tenant_ref', 'name', 'status'];

async function stubClipboard(page: Page) {
  await page.addInitScript(() => {
    const memory = { text: '' };
    Object.defineProperty(window, '__clipboard', { value: memory });
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          memory.text = text;
        },
        readText: async () => memory.text,
      },
    });
  });
}

const clipboard = (page: Page) =>
  page.evaluate(
    () =>
      (window as unknown as { __clipboard: { text: string } }).__clipboard.text,
  );

const setClipboard = (page: Page, text: string) =>
  page.evaluate(value => {
    (window as unknown as { __clipboard: { text: string } }).__clipboard.text =
      value;
  }, text);

/** Contagem da status bar ("3 de 3 linhas") */
const rowCount = (page: Page, text: string) =>
  expect(page.getByText(`${text} linhas`, { exact: true })).toBeVisible();

const text = async (g: Grid, index: number, column: string) => {
  await expect(g.value(index, column)).not.toBeEmpty();
  return (await g.value(index, column).textContent())!;
};

/** Os valores da linha, como o Cmd/Ctrl+C escreve */
const rowText = async (g: Grid, index: number, columns: string[]) => {
  const values: string[] = [];
  for (const column of columns) values.push(await text(g, index, column));
  return values.join(', ');
};

test.beforeEach(async ({ page }) => stubClipboard(page));

test.describe('duplicar linhas', () => {
  test('1. Cmd/Ctrl+D duplica a linha do cursor com o id como DEFAULT', async ({
    page,
  }) => {
    await openApp(page, { tabs: [USUARIOS] });
    const g = grid(page);
    await rowCount(page, '3 de 3');
    const nome = await text(g, 0, 'nome');

    await g.cell(0, 'nome').click();
    await page.keyboard.press(mod('d'));

    await expect(g.row(3)).toHaveAttribute('data-pending', 'added');
    await expect(g.pendingBar).toContainText('1 nova');
    // Serial: o banco gera; o resto vem da linha de origem
    await expect(g.value(3, 'id')).toHaveText('DEFAULT');
    await expect(g.value(3, 'nome')).toHaveText(nome);
    // O cursor vai para a cópia
    await expect(g.cell(3, 'nome')).toHaveClass(/ring-ring/);

    // email é único: a cópia precisa de outro antes de salvar
    await g.edit(3, 'email', 'copia@loja.dev');
    await g.save();
    await expect(g.pendingBar).toBeHidden();
    await rowCount(page, '4 de 4');
    // Id novo vem primeiro na ordem padrão (PK desc)
    await expect(g.value(0, 'nome')).toHaveText(nome);
    await expect(g.value(0, 'email')).toHaveText('copia@loja.dev');
    await expect(g.value(0, 'id')).not.toHaveText('DEFAULT');
  });

  test('2. o botão Duplicar copia as linhas selecionadas', async ({ page }) => {
    await openApp(page, { tabs: [USUARIOS] });
    const g = grid(page);
    await rowCount(page, '3 de 3');
    const nomes = [await text(g, 0, 'nome'), await text(g, 1, 'nome')];

    await g.selectRow(0);
    await g.selectRow(1, ['Shift']);
    await page.getByRole('button', { name: 'Duplicar' }).click();

    await expect(g.pendingBar).toContainText('2 novas');
    await expect(g.value(3, 'nome')).toHaveText(nomes[0]);
    await expect(g.value(4, 'nome')).toHaveText(nomes[1]);
    await expect(g.value(3, 'id')).toHaveText('DEFAULT');
    await expect(g.value(4, 'id')).toHaveText('DEFAULT');
  });

  test('3. "Nova linha" mostra DEFAULT nas células não tocadas', async ({
    page,
  }) => {
    await openApp(page, { tabs: [USUARIOS] });
    const g = grid(page);
    await rowCount(page, '3 de 3');

    await page.getByRole('button', { name: 'Nova linha' }).click();
    await expect(g.value(3, 'nome')).toHaveText('DEFAULT');
    // Definir NULL de propósito é outra coisa: a célula passa a mostrar NULL
    await g.openEditor(3, 'telefone');
    await page
      .getByRole('dialog')
      .getByRole('button', { name: /NULL/ })
      .click();
    await expect(g.value(3, 'telefone')).toHaveText('NULL');
  });
});

test.describe('copiar células', () => {
  test('4. uma célula vai crua para a área de transferência', async ({
    page,
  }) => {
    await openApp(page, { tabs: [EVENTS_1] });
    const g = grid(page);
    await rowCount(page, '3 de 3');
    const name = await text(g, 1, 'name');

    await g.cell(1, 'name').click();
    await page.keyboard.press(mod('c'));
    await expect.poll(() => clipboard(page)).toBe(name);
  });

  test('5. Shift+clique seleciona um bloco e copia separado por vírgula', async ({
    page,
  }) => {
    await openApp(page, { tabs: [EVENTS_1] });
    const g = grid(page);
    await rowCount(page, '3 de 3');
    const block = ['id', 'tenant_ref', 'name'];
    const expected = [
      await rowText(g, 0, block),
      await rowText(g, 1, block),
      await rowText(g, 2, block),
    ].join('\n');

    await g.cell(0, 'id').click();
    await g.cell(2, 'name').click({ modifiers: ['Shift'] });
    await expect(
      g.root.locator('[role=gridcell][aria-selected=true]'),
    ).toHaveCount(9);

    await page.keyboard.press(mod('c'));
    await expect.poll(() => clipboard(page)).toBe(expected);

    // Esc desfaz o bloco (o cursor fica)
    await page.keyboard.press('Escape');
    await expect(
      g.root.locator('[role=gridcell][aria-selected=true]'),
    ).toHaveCount(0);
  });

  test('6. Shift+setas estendem o bloco a partir do cursor', async ({
    page,
  }) => {
    await openApp(page, { tabs: [EVENTS_1] });
    const g = grid(page);
    await rowCount(page, '3 de 3');

    await g.cell(0, 'id').click();
    await page.keyboard.press('Shift+ArrowDown');
    await page.keyboard.press('Shift+ArrowRight');
    await expect(
      g.root.locator('[role=gridcell][aria-selected=true]'),
    ).toHaveCount(4);

    await page.keyboard.press(mod('c'));
    const expected = [
      await rowText(g, 0, ['id', 'tenant_ref']),
      await rowText(g, 1, ['id', 'tenant_ref']),
    ].join('\n');
    await expect.poll(() => clipboard(page)).toBe(expected);
  });
});

test.describe('copiar e colar linhas entre abas', () => {
  test('7. linhas inteiras coladas noutra aba viram linhas novas', async ({
    page,
  }) => {
    await openApp(page, { tabs: [EVENTS_1, EVENTS_2] });
    const g = grid(page);
    await rowCount(page, '3 de 3');
    const copied = [
      await rowText(g, 0, EVENTS_COLUMNS),
      await rowText(g, 1, EVENTS_COLUMNS),
    ];

    await g.selectRow(0);
    await g.selectRow(1, ['Shift']);
    await page.keyboard.press(mod('c'));
    await expect.poll(() => clipboard(page)).toBe(copied.join('\n'));

    await workspaceTab(page, 'tenant_00002').click();
    await rowCount(page, '3 de 3');
    await g.cell(0, 'name').click();
    await page.keyboard.press(mod('v'));

    await expect(g.pendingBar).toContainText('2 novas');
    // Noutra aba os valores vão como estão, PK incluída
    expect(await rowText(g, 3, EVENTS_COLUMNS)).toBe(copied[0]);
    expect(await rowText(g, 4, EVENTS_COLUMNS)).toBe(copied[1]);
  });

  test('8. colar na mesma aba deixa o id como DEFAULT', async ({ page }) => {
    await openApp(page, { tabs: [EVENTS_1] });
    const g = grid(page);
    await rowCount(page, '3 de 3');
    const name = await text(g, 0, 'name');

    await g.selectRow(0);
    await page.keyboard.press(mod('c'));
    await expect.poll(() => clipboard(page)).not.toBe('');
    await page.keyboard.press(mod('v'));

    await expect(g.pendingBar).toContainText('1 nova');
    await expect(g.value(3, 'id')).toHaveText('DEFAULT');
    await expect(g.value(3, 'name')).toHaveText(name);
  });

  test('9. com outra coisa na área de transferência, não cola', async ({
    page,
  }) => {
    await openApp(page, { tabs: [EVENTS_1] });
    const g = grid(page);
    await rowCount(page, '3 de 3');

    await g.selectRow(0);
    await page.keyboard.press(mod('c'));
    await expect.poll(() => clipboard(page)).not.toBe('');
    // Copiou outra coisa fora da grade depois
    await setClipboard(page, 'texto qualquer');
    await page.keyboard.press(mod('v'));

    await expect(page.getByText(/Para colar linhas, copie-as/)).toBeVisible();
    await expect(g.pendingBar).toBeHidden();
  });
});
