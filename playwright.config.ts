import { defineConfig, devices } from '@playwright/test';

/**
 * Testes de ponta a ponta do front (e2e/), rodando sobre o `pnpm dev:mock`: o
 * app inteiro num navegador comum, com o backend simulado no nível do IPC
 * (src/mocks/). O Rust real fica com o `cargo test`.
 *
 * WebKit é o motor do WKWebView, que o Tauri usa no macOS; Chromium é o mais
 * próximo do WebView2 do Windows.
 */
const ci = !!process.env.CI;

/** Menor janela do app (`minWidth`/`minHeight` no src-tauri/tauri.conf.json) */
const MIN_WINDOW = { width: 720, height: 480 };

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: ci,
  retries: ci ? 1 : 0,
  reporter: ci ? [['github'], ['html', { open: 'never' }]] : 'html',
  use: {
    baseURL: 'http://localhost:1430',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    { name: 'webkit', use: { ...devices['Desktop Safari'] } },
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    // A suíte inteira também na menor janela que o app aceita (minWidth/
    // minHeight do tauri.conf.json). O responsive.spec escolhe os próprios
    // tamanhos e roda só nos dois de cima.
    {
      name: 'chromium-min-window',
      use: { ...devices['Desktop Chrome'], viewport: MIN_WINDOW },
      testIgnore: /responsive\.spec\.ts/,
    },
  ],
  webServer: {
    command: 'pnpm dev:mock',
    url: 'http://localhost:1430',
    // Reaproveita um `pnpm dev:mock` já aberto na máquina
    reuseExistingServer: !ci,
    timeout: 60_000,
  },
});
