import {
  activeWorkspaceTab,
  closeTabDialog,
  columnHeader,
  expect,
  grid,
  mod,
  openApp,
  openFromTree,
  readEditor,
  savedSession,
  test,
  workspaceTab,
  type SeedTab,
} from './fixtures';

/**
 * Ciclo de vida das abas do documento: as alterações pendentes da grade são
 * da aba (sobrevivem à troca), fechar uma aba com pendências pede confirmação,
 * e o estado de cada aba (ordenação, filtro, colunas ocultas) é dela.
 */

const ENDERECOS: SeedTab = {
  kind: 'browse',
  id: 'browse-enderecos',
  title: 'enderecos',
  table: 'enderecos',
};
const CONSULTA: SeedTab = {
  kind: 'query',
  id: 'query-enderecos',
  title: 'Consulta',
  content: 'SELECT * FROM enderecos',
};
const RASCUNHO: SeedTab = {
  kind: 'query',
  id: 'query-rascunho',
  title: 'Rascunho',
  content: '',
};
const USUARIOS: SeedTab = {
  kind: 'browse',
  id: 'browse-usuarios',
  title: 'usuarios',
  table: 'usuarios',
};
const query = (title: string): SeedTab => ({
  kind: 'query',
  id: `query-${title}`,
  title,
  content: '',
});

/** Títulos das abas, na ordem da barra */
const tabTitles = (page: import('@playwright/test').Page) =>
  page
    .locator('[data-tauri-drag-region] > [role=tablist] [role=tab]')
    .allTextContents();

/** Primeira linha de `enderecos` num SELECT sem ORDER BY (ordem de inserção) */
const LOGRADOURO = 'Estrada do Campo Limpo, 2186';
const NOVO = 'Rua das Pendências, 1';

test.describe('pendências por aba', () => {
  test('1. sobrevivem à troca de aba (browse)', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS, RASCUNHO] });
    const g = grid(page);

    await expect(g.value(0, 'logradouro')).not.toBeEmpty();
    await g.edit(0, 'logradouro', NOVO);
    await expect(g.pendingTotal).toHaveText('1 alteração');

    await workspaceTab(page, 'Rascunho').click();
    await expect(g.root).toBeHidden();
    await workspaceTab(page, 'enderecos').click();

    await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
    await expect(g.pendingTotal).toHaveText('1 alteração');
  });

  test('2. sobrevivem à troca de aba (query editável)', async ({ page }) => {
    await openApp(page, { tabs: [CONSULTA, RASCUNHO] });
    const g = grid(page);

    await page.locator('.cm-content').click();
    await page.keyboard.press(mod('Enter'));
    await expect(g.value(0, 'logradouro')).toHaveText(LOGRADOURO);

    await g.edit(0, 'logradouro', NOVO);
    await g.selectRow(1);
    await page.keyboard.press('Backspace');
    await expect(g.pendingTotal).toHaveText('2 alterações');

    await workspaceTab(page, 'Rascunho').click();
    await expect(g.root).toBeHidden();
    await workspaceTab(page, 'Consulta').click();

    await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
    await expect(g.pendingTotal).toHaveText('2 alterações');
    await expect(g.pendingBar).toContainText('1 removida');
  });
});

test.describe('fechar aba com pendências', () => {
  test('3. pelo "x": cancelar mantém, confirmar fecha', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS, RASCUNHO] });
    const g = grid(page);
    await g.edit(0, 'logradouro', NOVO);

    const close = page.getByRole('button', { name: 'Fechar aba enderecos' });
    await close.click();
    await expect(closeTabDialog(page)).toBeVisible();
    await closeTabDialog(page)
      .getByRole('button', { name: 'Cancelar' })
      .click();

    await expect(closeTabDialog(page)).toBeHidden();
    await expect(activeWorkspaceTab(page)).toHaveText('enderecos');
    await expect(g.value(0, 'logradouro')).toHaveText(NOVO);
    await expect(g.pendingTotal).toHaveText('1 alteração');

    await close.click();
    await closeTabDialog(page)
      .getByRole('button', { name: 'Descartar e fechar' })
      .click();
    await expect(workspaceTab(page, 'enderecos')).toHaveCount(0);
    await expect(activeWorkspaceTab(page)).toHaveText('Rascunho');
  });

  test('4. pelo Cmd+W: cancelar mantém, confirmar fecha', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS, RASCUNHO] });
    const g = grid(page);
    await g.edit(0, 'logradouro', NOVO);

    await g.root.focus();
    await page.keyboard.press(mod('w'));
    await expect(closeTabDialog(page)).toBeVisible();
    await page.keyboard.press('Escape');

    await expect(closeTabDialog(page)).toBeHidden();
    await expect(workspaceTab(page, 'enderecos')).toHaveCount(1);
    await expect(g.pendingTotal).toHaveText('1 alteração');

    await g.root.focus();
    await page.keyboard.press(mod('w'));
    await closeTabDialog(page)
      .getByRole('button', { name: 'Descartar e fechar' })
      .click();
    await expect(workspaceTab(page, 'enderecos')).toHaveCount(0);
  });

  test('5. pelo clique do meio também pergunta', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS, RASCUNHO] });
    await grid(page).edit(0, 'logradouro', NOVO);

    await workspaceTab(page, 'enderecos').click({ button: 'middle' });
    await expect(closeTabDialog(page)).toBeVisible();
    await closeTabDialog(page)
      .getByRole('button', { name: 'Cancelar' })
      .click();
    await expect(workspaceTab(page, 'enderecos')).toHaveCount(1);
  });
});

test.describe('abrir, trocar e fechar abas', () => {
  test('6. fechar sem pendências não pergunta', async ({ page }) => {
    await openApp(page, { tabs: [ENDERECOS, RASCUNHO] });
    await expect(grid(page).value(0, 'logradouro')).not.toBeEmpty();

    await page.getByRole('button', { name: 'Fechar aba enderecos' }).click();
    await expect(workspaceTab(page, 'enderecos')).toHaveCount(0);
    await expect(closeTabDialog(page)).toHaveCount(0);

    await page.keyboard.press(mod('w'));
    await expect(page.getByText('Nenhuma aba aberta')).toBeVisible();
    await expect(closeTabDialog(page)).toHaveCount(0);
  });

  test('7. abrir a mesma tabela de novo reaproveita a aba', async ({
    page,
  }) => {
    await openApp(page, { tabs: [RASCUNHO] });

    await openFromTree(page, [
      'Loja (local)',
      'loja',
      'public',
      'Tabelas',
      'enderecos',
    ]);
    await expect(activeWorkspaceTab(page)).toHaveText('enderecos');

    await workspaceTab(page, 'Rascunho').click();
    await page
      .getByRole('tree')
      .getByText('enderecos', { exact: true })
      .click();
    await expect(activeWorkspaceTab(page)).toHaveText('enderecos');
    expect(await tabTitles(page)).toEqual(['Rascunho', 'enderecos']);
  });

  test('8. Cmd/Ctrl+1…9 vai à N-ésima aba', async ({ page }) => {
    await openApp(page, {
      tabs: [query('Um'), query('Dois'), query('Três')],
      active: 'query-Um',
    });

    await page.keyboard.press(mod('3'));
    await expect(activeWorkspaceTab(page)).toHaveText('Três');
    await page.keyboard.press(mod('2'));
    await expect(activeWorkspaceTab(page)).toHaveText('Dois');
    await page.keyboard.press(mod('1'));
    await expect(activeWorkspaceTab(page)).toHaveText('Um');

    // Sem a N-ésima aba, nada muda
    await page.keyboard.press(mod('9'));
    await expect(activeWorkspaceTab(page)).toHaveText('Um');
  });

  test('9. fechar a ativa ativa a vizinha da direita (ou a da esquerda, na ponta)', async ({
    page,
  }) => {
    await openApp(page, {
      tabs: [query('Um'), query('Dois'), query('Três'), query('Quatro')],
      active: 'query-Dois',
    });

    // A do meio: a da direita passa a ocupar a posição e fica ativa
    await page.getByRole('button', { name: 'Fechar aba Dois' }).click();
    await expect(activeWorkspaceTab(page)).toHaveText('Três');

    // Fechar outra que não a ativa não muda a ativa
    await page.getByRole('button', { name: 'Fechar aba Um' }).click();
    await expect(activeWorkspaceTab(page)).toHaveText('Três');

    // A última da barra: sem vizinha à direita, ativa a da esquerda
    await workspaceTab(page, 'Quatro').click();
    await page.keyboard.press(mod('w'));
    await expect(activeWorkspaceTab(page)).toHaveText('Três');
    expect(await tabTitles(page)).toEqual(['Três']);
  });

  test('10. fechar a última volta à tela vazia', async ({ page }) => {
    await openApp(page, { tabs: [RASCUNHO] });

    await page.getByRole('button', { name: 'Fechar aba Rascunho' }).click();
    await expect(page.getByText('Nenhuma aba aberta')).toBeVisible();
    await expect(page.getByRole('tab')).toHaveCount(0);
  });

  test('11. "Nova consulta" herda o servidor e o banco da aba ativa', async ({
    page,
  }) => {
    await openApp(page, {
      tabs: [
        RASCUNHO,
        {
          kind: 'query',
          id: 'query-notas',
          title: 'Notas',
          content: '',
          serverId: 3,
          database: 'main',
        },
      ],
      active: 'query-notas',
    });

    await page.getByRole('button', { name: 'Nova consulta' }).click();
    // O título da aba nova é o banco
    await expect(activeWorkspaceTab(page)).toHaveText('main');
    expect(await tabTitles(page)).toEqual(['Rascunho', 'Notas', 'main']);

    await expect
      .poll(async () => {
        const session = await savedSession(page);
        const tab = session?.tabs.find(t => t.id === session.activeTabId);
        return tab && { serverId: tab.serverId, database: tab.database };
      })
      .toEqual({ serverId: 3, database: 'main' });
  });
});

test.describe('estado de cada aba', () => {
  test('12. ordenação, filtro e colunas ocultas sobrevivem à troca e ao reload', async ({
    page,
  }) => {
    await openApp(page, { tabs: [ENDERECOS, USUARIOS] });
    const g = grid(page);
    const rows = (text: string) =>
      expect(page.getByText(`${text} linhas`, { exact: true })).toBeVisible();

    // Em `enderecos`: ordena por cidade, filtra e deixa só cidade e cep
    await columnHeader(page, 'cidade').click();
    await expect(columnHeader(page, 'cidade')).toHaveAttribute(
      'aria-sort',
      'ascending',
    );

    await page.locator('.cm-content').click();
    await page.keyboard.type('id <= 3');
    await page.keyboard.press('Enter');
    await rows('3 de 3');

    // A partir de "todas visíveis", marcar uma coluna mostra só ela
    await expect(columnHeader(page, 'logradouro')).toBeVisible();
    await page.getByRole('button', { name: /Colunas/ }).click();
    const selector = page.getByRole('dialog');
    await selector.getByText('cidade', { exact: true }).click();
    await selector.getByText('cep', { exact: true }).click();
    await page.keyboard.press('Escape');
    await expect(page.getByRole('button', { name: /Colunas/ })).toContainText(
      '2/8',
    );
    await expect(columnHeader(page, 'logradouro')).toHaveCount(0);

    // `usuarios` tem o próprio estado: sem filtro, ordem padrão, tudo visível
    await workspaceTab(page, 'usuarios').click();
    await expect(columnHeader(page, 'id')).toHaveAttribute(
      'aria-sort',
      'descending',
    );
    // Filtro vazio (o texto na tela seria o placeholder)
    await expect.poll(async () => (await readEditor(page))?.doc).toBe('');
    await expect(page.getByRole('button', { name: /Colunas/ })).toContainText(
      /(\d+)\/\1/,
    );

    const expectEnderecos = async () => {
      await expect(activeWorkspaceTab(page)).toHaveText('enderecos');
      await expect(columnHeader(page, 'cidade')).toHaveAttribute(
        'aria-sort',
        'ascending',
      );
      await expect(page.locator('.cm-content')).toHaveText('id <= 3');
      await rows('3 de 3');
      await expect(columnHeader(page, 'cep')).toBeVisible();
      await expect(columnHeader(page, 'logradouro')).toHaveCount(0);
      await expect(g.value(0, 'cidade')).not.toBeEmpty();
    };

    await workspaceTab(page, 'enderecos').click();
    await expectEnderecos();

    // A sessão é gravada com throttle (500 ms): espera o estado inteiro chegar
    // ao "disco" — uma gravação anterior pode já ter a aba ativa e o filtro,
    // mas não as colunas. No restore o id da aba de tabela é refeito
    // (`encodeNodeId`), por isso a busca pelo título.
    await expect
      .poll(async () => {
        const session = await savedSession(page);
        const active = session?.tabs.find(t => t.id === session.activeTabId);
        return (
          active && {
            title: active.title,
            whereExpr: active.whereExpr,
            sort: active.sort,
            hidden: (active.hiddenColumns as string[]).length,
          }
        );
      })
      .toEqual({
        title: 'enderecos',
        whereExpr: 'id <= 3',
        sort: [{ column: 'cidade', direction: 'asc' }],
        hidden: 6,
      });
    await page.reload();
    await expectEnderecos();
  });
});
