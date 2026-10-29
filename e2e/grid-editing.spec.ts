import {
  expect,
  failNextCommand,
  grid,
  mod,
  openApp,
  test,
  type Grid,
  type SeedTab,
} from './fixtures';
import type { Page } from '@playwright/test';

/**
 * Edição na grade (`ResultsTable`) de uma aba de tabela: as alterações ficam
 * pendentes — amarelo (editada), verde (nova), vermelho e riscado (removida) —
 * e só vão ao banco no "Salvar", todas juntas.
 */

const ENDERECOS: SeedTab = {
  kind: 'browse',
  id: 'browse-enderecos',
  title: 'enderecos',
  table: 'enderecos',
};

/**
 * Poucas linhas, todas na tela: a linha nova aparece depois das existentes, e
 * com a primeira página inteira (500) ela ficaria fora da janela virtualizada.
 * A cidade de teste traz de volta a linha inserida depois de salvar.
 */
const CIDADE_NOVA = 'Octopolis';
const POUCAS_LINHAS: SeedTab = {
  ...ENDERECOS,
  whereExpr: `id <= 3 OR cidade = '${CIDADE_NOVA}'`,
};

const NOVO = 'Rua das Pendências, 1';

const reload = (page: Page) =>
  page.getByRole('button', { name: 'Recarregar' }).click();

/** Contagem da status bar ("3 de 3 linhas") */
const rowCount = (page: Page, text: string) =>
  expect(page.getByText(`${text} linhas`, { exact: true })).toBeVisible();

/** O valor da célula antes de editar */
const original = async (g: Grid, index: number, column: string) => {
  await expect(g.value(index, column)).not.toBeEmpty();
  return (await g.value(index, column).textContent())!;
};

/** Nenhuma pendência: barra fechada e nenhuma célula ou linha tingida */
async function expectNoPending(g: Grid) {
  await expect(g.pendingBar).toBeHidden();
  await expect(g.root.locator('[data-modified], [data-pending]')).toHaveCount(
    0,
  );
}

test.describe('edição na grade', () => {
  test('1. editar e salvar persiste o valor', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS] });
    const g = grid(page);

    await g.edit(0, 'logradouro', NOVO);
    await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
    await expect(g.cell(0, 'logradouro')).toHaveAttribute(
      'data-modified',
      'true',
    );
    await expect(g.row(0)).toHaveAttribute('data-pending', 'modified');
    await expect(g.pendingTotal).toHaveText('1 alteração');
    await expect(g.pendingBar).toContainText('1 editada');

    await g.save();
    await expectNoPending(g);
    await expect(g.value(0, 'logradouro')).toHaveText(NOVO);

    await reload(page);
    await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
    await expectNoPending(g);
  });

  test('2. voltar ao original ou "Desfazer" no painel tira a pendência', async ({
    page,
  }) => {
    await openApp(page, { tabs: [ENDERECOS] });
    const g = grid(page);
    const LOGRADOURO = await original(g, 0, 'logradouro');

    // Editar de volta ao valor original não deixa pendência
    await g.edit(0, 'logradouro', NOVO);
    await expect(g.pendingTotal).toHaveText('1 alteração');
    await g.edit(0, 'logradouro', LOGRADOURO);
    await expectNoPending(g);

    // O "Desfazer" do painel de valor desfaz a célula sob o cursor
    await g.edit(0, 'logradouro', NOVO);
    await page.getByRole('button', { name: 'Valor' }).click();
    await g.cell(0, 'logradouro').click();
    await page.getByTitle('Desfazer a edição pendente desta célula').click();
    await expect(g.value(0, 'logradouro')).toHaveText(LOGRADOURO);
    await expectNoPending(g);
  });

  test('3. Esc no editor cancela a edição', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS] });
    const g = grid(page);
    const LOGRADOURO = await original(g, 0, 'logradouro');

    await g.openEditor(0, 'logradouro');
    await g.editor.fill(NOVO);
    await g.editor.press('Escape');

    await expect(g.editor).toBeHidden();
    await expect(g.value(0, 'logradouro')).toHaveText(LOGRADOURO);
    await expectNoPending(g);
  });

  test('4. descartar pelo botão e pelo atalho', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS] });
    const g = grid(page);
    const LOGRADOURO = await original(g, 0, 'logradouro');

    await g.edit(0, 'logradouro', NOVO);
    await g.discard();
    await expect(g.value(0, 'logradouro')).toHaveText(LOGRADOURO);
    await expectNoPending(g);

    // grid.discardChanges (Cmd/Ctrl+Z com o foco na grade)
    await g.edit(0, 'logradouro', NOVO);
    await g.root.focus();
    await page.keyboard.press(mod('z'));
    await expect(g.value(0, 'logradouro')).toHaveText(LOGRADOURO);
    await expectNoPending(g);
  });

  test('5. NULL é diferente de string vazia', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS] });
    const g = grid(page);

    // String vazia: a célula fica em branco, não "NULL"
    await g.edit(0, 'complemento', '');
    await expect(g.value(0, 'complemento')).toHaveText('');
    await g.save();
    await reload(page);
    await expect(g.value(0, 'complemento')).toHaveText('');
    await expectNoPending(g);

    // NULL pelo botão do editor: muda de '' para NULL, e é uma alteração
    await g.openEditor(0, 'complemento');
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'NULL', exact: true })
      .click();
    await expect(g.value(0, 'complemento')).toHaveText('NULL');
    await expect(g.pendingTotal).toHaveText('1 alteração');
    await g.save();
    await reload(page);
    await expect(g.value(0, 'complemento')).toHaveText('NULL');
    await expectNoPending(g);
  });

  test('6. inserir linha (verde) e salvar', async ({ page }) => {
    await openApp(page, { tabs: [POUCAS_LINHAS] });
    const g = grid(page);
    await rowCount(page, '3 de 3');

    await page.getByRole('button', { name: 'Nova linha' }).click();
    await expect(g.row(3)).toHaveAttribute('data-pending', 'added');
    // Não tocada: fica fora do INSERT, o banco aplica o default
    await expect(g.value(3, 'logradouro')).toHaveText('DEFAULT');
    await expect(g.pendingBar).toContainText('1 nova');

    await g.edit(3, 'logradouro', NOVO);
    await g.edit(3, 'cidade', CIDADE_NOVA);
    await expect(g.pendingTotal).toHaveText('1 alteração');

    await g.save();
    await expectNoPending(g);
    // A linha nova ganhou o próximo id: vem primeiro na ordem padrão
    await rowCount(page, '4 de 4');
    await expect(g.value(0, 'cidade')).toHaveText(CIDADE_NOVA);
    await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
  });

  test('7. remover linhas (vermelhas e riscadas) e salvar', async ({
    page,
  }) => {
    await openApp(page, { tabs: [POUCAS_LINHAS] });
    const g = grid(page);
    await rowCount(page, '3 de 3');
    const removido = await g.value(0, 'logradouro').textContent();

    await g.selectRow(0);
    await g.selectRow(1, ['Shift']);
    await page.keyboard.press('Backspace');

    for (const index of [0, 1]) {
      await expect(g.row(index)).toHaveAttribute('data-pending', 'removed');
      await expect(g.value(index, 'logradouro')).toHaveCSS(
        'text-decoration-line',
        'line-through',
      );
    }
    await expect(g.row(2)).not.toHaveAttribute('data-pending');
    await expect(g.pendingTotal).toHaveText('2 alterações');
    await expect(g.pendingBar).toContainText('2 removidas');

    await g.save();
    await expectNoPending(g);
    await rowCount(page, '1 de 1');
    await expect(g.value(0, 'id')).toHaveText('1');
    await expect(g.root.getByText(removido!)).toHaveCount(0);
  });

  test('8. edição, inserção e remoção num mesmo salvar', async ({ page }) => {
    await openApp(page, { tabs: [POUCAS_LINHAS] });
    const g = grid(page);
    await rowCount(page, '3 de 3');

    // Linhas: id 3, 2, 1. Edita a do id 3 e remove a do id 1.
    await g.edit(0, 'logradouro', NOVO);
    await g.selectRow(2);
    await page.keyboard.press('Backspace');
    await page.getByRole('button', { name: 'Nova linha' }).click();
    await g.edit(3, 'cidade', CIDADE_NOVA);

    await expect(g.pendingTotal).toHaveText('3 alterações');
    await expect(g.pendingBar).toContainText('1 editada');
    await expect(g.pendingBar).toContainText('1 nova');
    await expect(g.pendingBar).toContainText('1 removida');

    await g.save();
    await expectNoPending(g);
    await rowCount(page, '3 de 3');
    await expect(g.value(0, 'cidade')).toHaveText(CIDADE_NOVA);
    await expect(g.value(1, 'id')).toHaveText('3');
    await expect(g.value(1, 'logradouro')).toHaveText(NOVO);
    await expect(g.value(2, 'id')).toHaveText('2');
  });

  test('9. erro ao salvar mantém as pendências; o salvar seguinte funciona', async ({
    page,
  }) => {
    await openApp(page, { tabs: [ENDERECOS] });
    const g = grid(page);

    await g.edit(0, 'logradouro', NOVO);
    await failNextCommand(page);
    await g.save();

    await expect(page.getByText(/connection refused/)).toBeVisible();
    await expect(g.pendingTotal).toHaveText('1 alteração');
    await expect(g.cell(0, 'logradouro')).toHaveAttribute(
      'data-modified',
      'true',
    );

    await g.save();
    await expectNoPending(g);
    await reload(page);
    await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
  });

  test('10. Cmd/Ctrl+S salva', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS] });
    const g = grid(page);

    await g.edit(0, 'logradouro', NOVO);
    await g.root.focus();
    await page.keyboard.press(mod('s'));

    await expectNoPending(g);
    await reload(page);
    await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
  });
});
