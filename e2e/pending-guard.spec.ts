import type { Page } from '@playwright/test';
import {
  columnHeader,
  expect,
  grid,
  mod,
  openApp,
  pendingChangesDialog,
  readEditor,
  test,
  type Grid,
  type SeedTab,
} from './fixtures';

/**
 * Ordenar, filtrar, recarregar ou reexecutar a query troca os dados da grade.
 * Com alterações pendentes, isso pergunta antes: Salvar (grava e aplica),
 * Descartar (descarta e aplica) ou Cancelar (não aplica nada e mantém as
 * pendências). Sem pendências, aplica direto.
 */

/** Ids 3, 2 e 1, nessa ordem (PK desc) */
const TRES_LINHAS: SeedTab = {
  kind: 'browse',
  id: 'browse-enderecos',
  title: 'enderecos',
  table: 'enderecos',
  whereExpr: 'id <= 3',
};
const CONSULTA: SeedTab = {
  kind: 'query',
  id: 'query-enderecos',
  title: 'Consulta',
  content: 'SELECT * FROM enderecos',
};

const NOVO = 'Rua das Pendências, 1';

type Choice = 'Salvar' | 'Descartar' | 'Cancelar';
const CHOICES: Choice[] = ['Salvar', 'Descartar', 'Cancelar'];

const rowCount = (page: Page, text: string) =>
  expect(page.getByText(`${text} linhas`, { exact: true })).toBeVisible();

async function choose(page: Page, choice: Choice) {
  const dialog = pendingChangesDialog(page);
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText('1 alteração');
  await dialog.getByRole('button', { name: choice, exact: true }).click();
  await expect(dialog).toBeHidden();
}

/** Depois de Salvar ou Descartar não sobra pendência; no Cancelar ela fica */
async function expectPending(g: Grid, choice: Choice) {
  if (choice === 'Cancelar') {
    await expect(g.pendingTotal).toHaveText('1 alteração');
  } else {
    await expect(g.pendingBar).toBeHidden();
    await expect(g.root.locator('[data-modified]')).toHaveCount(0);
  }
}

/** Troca o texto do filtro WHERE e aplica com Enter */
async function applyFilter(page: Page, text: string) {
  await page.locator('.cm-content').click();
  await page.keyboard.press(mod('a'));
  // Termina em número: o autocomplete não abre e o Enter aplica
  await page.keyboard.type(text);
  await page.keyboard.press('Enter');
}

test.describe('ordenar com pendências', () => {
  for (const choice of CHOICES) {
    test(`${choice}`, async ({ page }) => {
      await openApp(page, { tabs: [TRES_LINHAS] });
      const g = grid(page);
      await rowCount(page, '3 de 3');
      await g.edit(0, 'logradouro', NOVO);

      await columnHeader(page, 'cidade').click();
      await choose(page, choice);
      await expectPending(g, choice);

      if (choice === 'Cancelar') {
        // A ordenação continua a de antes, com a edição no lugar
        await expect(columnHeader(page, 'id')).toHaveAttribute(
          'aria-sort',
          'descending',
        );
        await expect(columnHeader(page, 'cidade')).not.toHaveAttribute(
          'aria-sort',
        );
        await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
        return;
      }

      await expect(columnHeader(page, 'cidade')).toHaveAttribute(
        'aria-sort',
        'ascending',
      );
      await rowCount(page, '3 de 3');
      // Salvo, o valor novo está no banco; descartado, sumiu
      await expect(g.root.getByText(NOVO)).toHaveCount(
        choice === 'Salvar' ? 1 : 0,
      );
    });
  }
});

test.describe('filtrar com pendências', () => {
  for (const choice of CHOICES) {
    test(`${choice}`, async ({ page }) => {
      await openApp(page, { tabs: [TRES_LINHAS] });
      const g = grid(page);
      await rowCount(page, '3 de 3');
      // Linha do id 2: continua no resultado depois do filtro `id <= 2`
      await expect(g.value(1, 'id')).toHaveText('2');
      await g.edit(1, 'logradouro', NOVO);

      await applyFilter(page, 'id <= 2');
      await choose(page, choice);
      await expectPending(g, choice);

      if (choice === 'Cancelar') {
        // O filtro aplicado continua o de antes; o texto digitado fica no campo
        await rowCount(page, '3 de 3');
        await expect(g.value(1, 'logradouro')).toHaveText(NOVO);
        expect((await readEditor(page))?.doc).toBe('id <= 2');
        return;
      }

      await rowCount(page, '2 de 2');
      await expect(g.value(0, 'id')).toHaveText('2');
      if (choice === 'Salvar') {
        await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
      } else {
        await expect(g.value(0, 'logradouro')).not.toHaveText(NOVO);
      }
    });
  }
});

test.describe('recarregar com pendências', () => {
  for (const choice of CHOICES) {
    test(`${choice}`, async ({ page }) => {
      await openApp(page, { tabs: [TRES_LINHAS] });
      const g = grid(page);
      await rowCount(page, '3 de 3');
      const original = await g.value(0, 'logradouro').textContent();
      await g.edit(0, 'logradouro', NOVO);

      await page.getByRole('button', { name: 'Recarregar' }).click();
      await choose(page, choice);
      await expectPending(g, choice);

      await expect(g.value(0, 'logradouro')).toHaveText(
        choice === 'Descartar' ? original! : NOVO,
      );
      if (choice === 'Salvar') {
        // Gravado mesmo: recarregar de novo, agora sem pendências, não pergunta
        await page.getByRole('button', { name: 'Recarregar' }).click();
        await expect(pendingChangesDialog(page)).toHaveCount(0);
        await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
      }
    });
  }
});

test.describe('reexecutar a query com pendências', () => {
  for (const choice of CHOICES) {
    test(`${choice}`, async ({ page }) => {
      await openApp(page, { tabs: [CONSULTA] });
      const g = grid(page);
      const run = async () => {
        await page.locator('.cm-content').click();
        await page.keyboard.press(mod('Enter'));
      };

      await run();
      await expect(g.value(0, 'logradouro')).not.toBeEmpty();
      const original = await g.value(0, 'logradouro').textContent();
      await g.edit(0, 'logradouro', NOVO);

      await run();
      await choose(page, choice);
      await expectPending(g, choice);
      await expect(g.value(0, 'logradouro')).toHaveText(
        choice === 'Descartar' ? original! : NOVO,
      );
    });
  }
});

test('sem pendências, ordenar, filtrar, recarregar e reexecutar não perguntam', async ({
  page,
}) => {
  await openApp(page, { tabs: [TRES_LINHAS, CONSULTA] });
  const g = grid(page);
  await rowCount(page, '3 de 3');

  await columnHeader(page, 'cidade').click();
  await expect(columnHeader(page, 'cidade')).toHaveAttribute(
    'aria-sort',
    'ascending',
  );
  await applyFilter(page, 'id <= 2');
  await rowCount(page, '2 de 2');
  await page.getByRole('button', { name: 'Recarregar' }).click();
  await rowCount(page, '2 de 2');

  await page
    .locator('[data-tauri-drag-region] > [role=tablist]')
    .getByRole('tab', { name: 'Consulta' })
    .click();
  await page.locator('.cm-content').click();
  await page.keyboard.press(mod('Enter'));
  await expect(g.value(0, 'logradouro')).not.toBeEmpty();
  await page.keyboard.press(mod('Enter'));
  await expect(g.value(0, 'logradouro')).not.toBeEmpty();

  await expect(pendingChangesDialog(page)).toHaveCount(0);
});
