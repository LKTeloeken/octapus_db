import type { Locator, Page } from '@playwright/test';
import {
  activeWorkspaceTab,
  expect,
  mod,
  openApp,
  openFromTree,
  test,
  type SeedTab,
} from './fixtures';

/**
 * Janelas pequenas: da menor que o app aceita (720×480, o `minWidth`/
 * `minHeight` do tauri.conf.json) até o limite da janela compacta (1040 px,
 * `COMPACT_WINDOW_PX`). Nada da barra lateral, das barras de ferramentas ou da
 * linha de abas pode sumir, ficar com largura zero ou ser coberto por outro
 * botão; abaixo de 1040 px o assistente abre como a janela inteira.
 */

const SIZES = [
  { width: 720, height: 480 },
  { width: 800, height: 600 },
  { width: 1040, height: 600 },
];

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

/** Visível, com largura e inteiro dentro da janela (nada cortado na borda) */
async function expectOnScreen(page: Page, locator: Locator) {
  await expect(locator).toBeVisible();
  const box = await locator.boundingBox();
  const { width } = page.viewportSize()!;
  expect(box, 'sem caixa').not.toBeNull();
  expect(box!.width).toBeGreaterThan(0);
  expect(box!.x).toBeGreaterThanOrEqual(0);
  expect(box!.x + box!.width).toBeLessThanOrEqual(width);
}

/** `item` inteiro dentro da área visível de `container` (lista que rola) */
async function expectWithin(container: Locator, item: Locator) {
  await expect(async () => {
    const outer = (await container.boundingBox())!;
    const inner = (await item.boundingBox())!;
    expect(inner.x).toBeGreaterThanOrEqual(outer.x - 1);
    expect(inner.x + inner.width).toBeLessThanOrEqual(
      outer.x + outer.width + 1,
    );
  }).toPass();
}

/**
 * Elementos com overflow escondido que rolaram por dentro (o foco num campo
 * espremido rolava o painel e sumia o começo dele, sem barra para voltar)
 */
const hiddenScrolls = (page: Page): Promise<string[]> =>
  page.evaluate(() =>
    Array.from(document.querySelectorAll<HTMLElement>('body *'))
      .filter(
        el =>
          getComputedStyle(el).overflowX === 'hidden' &&
          el.scrollLeft > 0 &&
          !el.closest('.cm-editor'),
      )
      .map(el => `<${el.tagName.toLowerCase()} class="${el.className}">`),
  );

const workspaceTabList = (page: Page) =>
  page.locator('[data-tauri-drag-region] > [role=tablist]');

/** Quanto o documento passa da janela (> 0: a página inteira rola) */
const documentOverflow = (page: Page): Promise<number> =>
  page.evaluate(
    () => document.scrollingElement!.scrollHeight - window.innerHeight,
  );

const crashScreen = (page: Page) => page.getByText(/^Algo deu errado/);

/** Escolhe o Postgres `loja` como alvo da conversa */
async function targetLoja(panel: Locator) {
  await panel.getByRole('combobox', { name: 'Servidor' }).click();
  await panel.page().getByRole('option', { name: 'Loja (local)' }).click();
  await expect(panel.getByRole('combobox', { name: 'Database' })).toHaveText(
    'loja',
  );
}

/** Pergunta ao assistente (Enter envia) */
async function ask(panel: Locator, question: string) {
  await panel.getByRole('textbox', { name: 'Pergunta ao assistente' }).click();
  await panel.page().keyboard.type(question);
  await panel.page().keyboard.press('Enter');
}

for (const size of SIZES) {
  test.describe(`janela ${size.width}×${size.height}`, () => {
    test.use({ viewport: size });

    test('a árvore mostra os nomes', async ({ page }) => {
      await openApp(page);

      await openFromTree(page, ['Loja (local)', 'loja', 'public', 'Tabelas']);
      const tree = page.getByRole('tree');
      await expectOnScreen(page, tree.getByText('enderecos', { exact: true }));
      await tree.getByText('enderecos', { exact: true }).click();
      await expect(activeWorkspaceTab(page)).toHaveText('enderecos');
    });

    test('browse: barra e grade cabem e o filtro WHERE é clicável', async ({
      page,
    }) => {
      await openApp(page, { tabs: [ENDERECOS] });
      await expect(page.getByText(/linhas$/)).toBeVisible();

      const table = page
        .getByRole('navigation', { name: 'Local' })
        .getByText('enderecos');
      await expectOnScreen(page, table);
      for (const name of [/Colunas/, 'Valor', 'Recarregar']) {
        await expectOnScreen(page, page.getByRole('button', { name }));
      }
      for (const name of ['Nova linha', 'Exportar']) {
        await expectOnScreen(page, page.getByRole('button', { name }));
      }

      // O clique falha se outro botão cobrir o campo (o "Colunas" cobria)
      await page.locator('.cm-content').click();
      await page.keyboard.type('id = 2');
      await page.keyboard.press('Enter');
      await expect(page.getByText('1 de 1 linhas')).toBeVisible();

      await expectOnScreen(page, table);
      expect(await hiddenScrolls(page)).toEqual([]);
    });

    test('query: barra do editor e dos resultados cabem', async ({ page }) => {
      await openApp(page, { tabs: [CONSULTA] });

      const run = page.getByRole('button', { name: /Executar/ });
      await expectOnScreen(page, run);
      await run.click();
      await expect(page.getByText(/linhas$/)).toBeVisible();

      await expectOnScreen(page, page.getByRole('tab', { name: 'Resultados' }));
      await expectOnScreen(page, page.getByRole('button', { name: /Colunas/ }));
      await expectOnScreen(
        page,
        page.getByRole('button', { name: 'Exportar' }),
      );
      expect(await hiddenScrolls(page)).toEqual([]);
    });

    test('a aba ativa fica à vista na linha de abas', async ({ page }) => {
      const tabs: SeedTab[] = Array.from({ length: 12 }, (_, index) => ({
        kind: 'query',
        id: `query-${index + 1}`,
        title: `Consulta número ${index + 1}`,
        content: '',
      }));
      await openApp(page, { tabs, active: 'query-12' });

      const list = workspaceTabList(page);
      await expect(activeWorkspaceTab(page)).toHaveText('Consulta número 12');
      await expectWithin(list, activeWorkspaceTab(page));

      await page.keyboard.press(mod('1'));
      await expect(activeWorkspaceTab(page)).toHaveText('Consulta número 1');
      await expectWithin(list, activeWorkspaceTab(page));

      // Assistente e Comandos continuam na janela, mesmo com tantas abas
      await expectOnScreen(
        page,
        page.getByRole('button', { name: /^Comandos/ }),
      );
      await expectOnScreen(
        page,
        page.getByRole('button', { name: /^Assistente/ }),
      );
    });
  });
}

test.describe('assistente', () => {
  test.describe('janela compacta (abaixo de 1040 px)', () => {
    test.use({ viewport: { width: 1039, height: 600 } });

    test('abre como a janela inteira e fechar volta às abas', async ({
      page,
    }) => {
      await openApp(page, { tabs: [CONSULTA] });
      await page.getByRole('button', { name: /^Assistente/ }).click();

      const panel = page.getByRole('region', { name: 'Assistente' });
      await expect(panel).toBeVisible();
      await expect(workspaceTabList(page)).toBeHidden();
      // Sem como sair do modo agente: o painel não caberia ao lado
      await expect(
        panel.getByRole('button', { name: /modo agente/i }),
      ).toHaveCount(0);

      await panel.getByRole('button', { name: 'Fechar o assistente' }).click();
      await expect(panel).toBeHidden();
      await expect(activeWorkspaceTab(page)).toHaveText('Consulta');
    });
  });

  test.describe('janela larga', () => {
    test.use({ viewport: { width: 1280, height: 720 } });

    test('abre ao lado das abas', async ({ page }) => {
      await openApp(page, { tabs: [CONSULTA] });
      await page.getByRole('button', { name: /^Assistente/ }).click();

      const panel = page.getByRole('region', { name: 'Assistente' });
      await expect(panel).toBeVisible();
      await expect(activeWorkspaceTab(page)).toBeVisible();
      await expect(
        panel.getByRole('button', { name: 'Modo agente' }),
      ).toBeVisible();
    });

    test('a pergunta do assistente no fim de uma conversa longa não faz a página rolar', async ({
      page,
    }) => {
      await openApp(page, { tabs: [CONSULTA], assistant: true });
      await page.getByRole('button', { name: /^Assistente/ }).click();
      const panel = page.getByRole('region', { name: 'Assistente' });
      await targetLoja(panel);

      const answered = panel.getByText('A resposta ajudou?');
      await ask(panel, 'quantos usuarios temos?');
      await expect(answered).toHaveCount(1);
      await ask(panel, 'e quantos usuarios no total?');
      await expect(answered).toHaveCount(2);

      // "vendas" → o assistente pergunta qual data vale e espera; o campo da
      // resposta fica abaixo da dobra da conversa
      await ask(panel, 'quantas vendas tivemos no mes passado?');
      const reply = panel.getByRole('textbox', { name: 'Sua resposta' });
      await expect(reply).toBeVisible();
      expect(await documentOverflow(page)).toBeLessThanOrEqual(0);

      await reply.fill('data do pedido');
      await reply.press('Enter');
      await expect(answered).toHaveCount(3);
      expect(await documentOverflow(page)).toBeLessThanOrEqual(0);
    });
  });

  test.describe('cruzar a janela compacta com o painel aberto', () => {
    test.use({ viewport: { width: 1400, height: 850 } });

    test('encolher de uma vez vira modo agente sem quebrar o app', async ({
      page,
    }) => {
      await openApp(page, { tabs: [CONSULTA], assistant: true });
      await page.getByRole('button', { name: /^Assistente/ }).click();
      const panel = page.getByRole('region', { name: 'Assistente' });
      await expect(
        panel.getByRole('button', { name: 'Modo agente' }),
      ).toBeVisible();

      // Sair do maximizado / organizar janelas: um salto só de largura
      await page.setViewportSize({ width: 1000, height: 850 });
      await expect(panel).toBeVisible();
      await expect(workspaceTabList(page)).toBeHidden();
      await expect(crashScreen(page)).toHaveCount(0);

      await page.setViewportSize({ width: 1400, height: 850 });
      await expect(
        panel.getByRole('button', { name: 'Modo agente' }),
      ).toBeVisible();
      await expect(activeWorkspaceTab(page)).toBeVisible();
      await expect(crashScreen(page)).toHaveCount(0);
    });

    test('com a barra lateral no mínimo, cruzar 1040 px aos poucos não quebra o app', async ({
      page,
    }) => {
      await openApp(page, { tabs: [CONSULTA], assistant: true });
      await page.getByRole('button', { name: /^Assistente/ }).click();
      const panel = page.getByRole('region', { name: 'Assistente' });
      await expect(panel).toBeVisible();

      // Arrasta a alça da barra lateral até o mínimo (200 px)
      const handle = page.locator('[data-panel-resize-handle-id]').first();
      const box = (await handle.boundingBox())!;
      const y = box.y + box.height / 2;
      await page.mouse.move(box.x + box.width / 2, y);
      await page.mouse.down();
      await page.mouse.move(box.x - 150, y, { steps: 10 });
      await page.mouse.up();
      const sidebar = (await page.locator('#sidebar').boundingBox())!;
      expect(sidebar.width).toBeLessThan(205);

      for (const width of [1100, 1050, 1039]) {
        await page.setViewportSize({ width, height: 850 });
      }
      await expect(workspaceTabList(page)).toBeHidden();
      await expect(panel).toBeVisible();
      await expect(crashScreen(page)).toHaveCount(0);
    });
  });
});
