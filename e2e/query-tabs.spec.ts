import {
  activeWorkspaceTab,
  captureFirstFrameAfterClick,
  expect,
  firstFrameText,
  openApp,
  readEditor,
  test,
  workspaceTab,
  type SeedTab,
} from './fixtures';

/**
 * Cada aba de query tem o seu editor: texto, seleção, histórico de undo e
 * rolagem não vazam de uma aba para outra, e a troca é imediata.
 *
 * Regressões do bug em que um único CodeMirror era reaproveitado entre as abas
 * (a troca substituía o doc inteiro, mapeando a seleção e dividindo o
 * histórico, e o `@uiw/react-codemirror` segurava a troca por 200 ms).
 */

// Comentário SQL: digitar não abre o autocomplete
const LONG_TEXT = Array.from(
  { length: 200 },
  (_, i) => `-- linha ${String(i + 1).padStart(3, '0')} da aba A`,
).join('\n');

const TAB_A: SeedTab = {
  kind: 'query',
  id: 'query-a',
  title: 'Aba A',
  content: LONG_TEXT,
};
const TAB_B: SeedTab = {
  kind: 'query',
  id: 'query-b',
  title: 'Aba B',
  content: '',
};
const ENDERECOS: SeedTab = {
  kind: 'browse',
  id: 'browse-enderecos',
  title: 'enderecos',
  table: 'enderecos',
};

const TEXT_B = '-- texto da aba B';

test.describe('abas de query', () => {
  test('1. cada aba mantém o próprio texto', async ({ page }) => {
    await openApp(page, { tabs: [TAB_A, TAB_B], active: 'query-b' });

    await page.locator('.cm-content').click();
    await page.keyboard.type(TEXT_B);

    await workspaceTab(page, 'Aba A').click();
    await expect
      .poll(async () => (await readEditor(page))?.doc)
      .toBe(LONG_TEXT);

    await workspaceTab(page, 'Aba B').click();
    await expect.poll(async () => (await readEditor(page))?.doc).toBe(TEXT_B);
  });

  test('2. a seleção não vaza e volta ao retornar', async ({ page }) => {
    await openApp(page, { tabs: [TAB_A, TAB_B] });

    await page.locator('.cm-content').click();
    await page.keyboard.press('ControlOrMeta+Home');
    for (let i = 0; i < 5; i++) await page.keyboard.press('ArrowRight');
    for (let i = 0; i < 15; i++) await page.keyboard.press('Shift+ArrowRight');
    expect((await readEditor(page))?.selection).toEqual([
      { anchor: 5, head: 20 },
    ]);

    // Bug antigo: B herdava a seleção mapeada ([19, 0], tudo selecionado)
    await workspaceTab(page, 'Aba B').click();
    await expect.poll(async () => (await readEditor(page))?.doc).toBe('');
    expect((await readEditor(page))?.selection).toEqual([
      { anchor: 0, head: 0 },
    ]);

    await workspaceTab(page, 'Aba A').click();
    await expect
      .poll(async () => (await readEditor(page))?.selection)
      .toEqual([{ anchor: 5, head: 20 }]);
  });

  test('3. o undo de uma aba não traz o texto da outra', async ({ page }) => {
    await openApp(page, { tabs: [TAB_A, TAB_B] });

    await workspaceTab(page, 'Aba B').click();
    await page.locator('.cm-content').click();

    // Logo após a troca, sem nada digitado: o undo não tem o que desfazer
    await page.keyboard.press('ControlOrMeta+z');
    expect((await readEditor(page))?.doc).toBe('');

    await page.keyboard.type(TEXT_B);
    await page.keyboard.press('ControlOrMeta+z');
    expect((await readEditor(page))?.doc).toBe('');

    // O desfeito em B não mexeu no conteúdo de A
    await workspaceTab(page, 'Aba A').click();
    await expect
      .poll(async () => (await readEditor(page))?.doc)
      .toBe(LONG_TEXT);
  });

  test('4. trocar logo depois de digitar é imediato e não perde texto', async ({
    page,
  }) => {
    await openApp(page, { tabs: [TAB_A, TAB_B], active: 'query-b' });

    await page.locator('.cm-content').click();
    await page.keyboard.type(TEXT_B);

    // Sem esperar: o clique sai dentro da janela de 200 ms do bug antigo
    await captureFirstFrameAfterClick(page);
    await workspaceTab(page, 'Aba A').click();
    expect(await firstFrameText(page)).toContain('-- linha 001 da aba A');

    await workspaceTab(page, 'Aba B').click();
    await expect.poll(async () => (await readEditor(page))?.doc).toBe(TEXT_B);
  });

  test('5. vindo de uma aba de tabela, o texto aparece no primeiro quadro', async ({
    page,
  }) => {
    await openApp(page, { tabs: [ENDERECOS, TAB_A] });
    await expect(page.getByText(/linhas$/)).toBeVisible();

    await captureFirstFrameAfterClick(page);
    await workspaceTab(page, 'Aba A').click();
    expect(await firstFrameText(page)).toContain('-- linha 001 da aba A');
  });

  test('6. a rolagem de cada aba é preservada', async ({ page }) => {
    await openApp(page, { tabs: [TAB_A, TAB_B] });

    const editor = page.locator('.cm-scroller');
    await editor.hover();
    await page.mouse.wheel(0, 100_000);
    await expect
      .poll(async () => (await readEditor(page))?.scrollTop)
      .toBeGreaterThan(0);
    const scrolled = (await readEditor(page))!.scrollTop;

    await workspaceTab(page, 'Aba B').click();
    await expect.poll(async () => (await readEditor(page))?.doc).toBe('');

    await workspaceTab(page, 'Aba A').click();
    await expect
      .poll(async () => (await readEditor(page))?.scrollTop)
      .toBe(scrolled);
  });

  test('7. fechar, abrir outra e recarregar não vaza estado', async ({
    page,
  }) => {
    await openApp(page, { tabs: [TAB_A, TAB_B], active: 'query-b' });

    await page.locator('.cm-content').click();
    await page.keyboard.type(TEXT_B);

    await page.getByRole('button', { name: 'Fechar aba Aba B' }).click();
    await expect(workspaceTab(page, 'Aba B')).toHaveCount(0);
    await expect(activeWorkspaceTab(page)).toHaveText('Aba A');

    // A nova aba nasce vazia, sem herdar o texto da fechada
    await page.getByRole('button', { name: 'Nova consulta' }).click();
    await expect(activeWorkspaceTab(page)).toHaveText('loja');
    await expect.poll(async () => (await readEditor(page))?.doc).toBe('');
    await page.locator('.cm-content').click();
    await page.keyboard.type('-- aba nova');

    // A sessão é gravada com throttle de 500 ms: espera chegar ao "disco"
    await expect
      .poll(() =>
        page.evaluate(() => localStorage.getItem('octapus-mock-session')),
      )
      .toContain('-- aba nova');
    await page.reload();

    await expect(activeWorkspaceTab(page)).toHaveText('loja');
    await expect
      .poll(async () => (await readEditor(page))?.doc)
      .toBe('-- aba nova');
    await workspaceTab(page, 'Aba A').click();
    await expect
      .poll(async () => (await readEditor(page))?.doc)
      .toBe(LONG_TEXT);
    await expect(workspaceTab(page, 'Aba B')).toHaveCount(0);
  });

  test('9. o filtro WHERE aplica com Enter e descarta com Esc', async ({
    page,
  }) => {
    await openApp(page, { tabs: [ENDERECOS] });
    await expect(page.getByText(/linhas$/)).toBeVisible();

    const filter = page.locator('.cm-content');
    await filter.click();
    // Termina em número: o autocomplete não abre e o Enter aplica
    await page.keyboard.type('id = 2');
    await page.keyboard.press('Enter');
    await expect(page.getByText('1 de 1 linhas')).toBeVisible();

    // Esc sem popup volta ao filtro aplicado. O autocomplete pode abrir um
    // instante depois de digitar, e aí o primeiro Esc só o fecha: repete até
    // descartar (um Esc a mais, já descartado, não muda nada).
    await page.keyboard.type(' AND false');
    await expect(filter).toHaveText('id = 2 AND false');
    await expect(async () => {
      await page.keyboard.press('Escape');
      await expect(filter).toHaveText('id = 2', { timeout: 500 });
    }).toPass();
    await expect(page.getByText('1 de 1 linhas')).toBeVisible();
  });
});
