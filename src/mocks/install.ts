import { createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { mockIPC } from '@tauri-apps/api/mocks';
import { MockDevPanel } from './dev-panel';
import { route } from './router';

const PANEL_ID = 'octapus-mock-panel';

/**
 * Instala o backend simulado no nível do IPC.
 *
 * `mockIPC` preenche `window.__TAURI_INTERNALS__.invoke` e `.transformCallback`,
 * então tudo que depende do runtime do Tauri passa a funcionar sem o Rust:
 * o `invoke` de src/api/client.ts, o `Channel` de src/api/query.ts e o
 * `check()` do plugin-updater. Nada em src/api/, src/queries/ ou nas features
 * precisa saber que está em modo mock.
 *
 * Detalhe importante para quem for mexer nos handlers: como o `invoke` interno
 * é substituído, os argumentos chegam **sem serialização** — `args.messages` é
 * a instância de `Channel`, não a string `__CHANNEL__:id`.
 */
export function installMocks(): void {
  mockIPC((command, payload) =>
    route(command, (payload ?? {}) as Record<string, unknown>),
  );

  mountDevPanel();

  console.info(
    '%c[octapus-mock]%c backend simulado ativo — nenhum comando chega ao Rust',
    'color:#a78bfa;font-weight:bold',
    'color:inherit',
  );
}

/**
 * O painel vive num root próprio pendurado no body, fora do `#root`: assim
 * app.tsx não precisa de nenhuma condicional de modo mock.
 */
function mountDevPanel() {
  if (document.getElementById(PANEL_ID)) return;

  const container = document.createElement('div');
  container.id = PANEL_ID;
  document.body.appendChild(container);

  createRoot(container).render(createElement(MockDevPanel));
}
