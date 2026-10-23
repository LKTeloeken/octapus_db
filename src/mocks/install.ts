import { createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { mockCatalog } from './catalog';
import { resetDataset } from './data';
import { MockDevPanel } from './dev-panel';
import { mockEmit } from './events';
import { useMockStore } from './mock-store';
import { route } from './router';

const PANEL_ID = 'octapus-mock-panel';

/**
 * Controle do mock para os testes de ponta a ponta (e2e/, Playwright): o mesmo
 * que o painel faz, sem precisar clicar nele.
 */
export interface MockControl {
  /** Latência, injeção de erro, modo vazio, update fake */
  store: typeof useMockStore;
  /** Dispara um evento global, como o `emit` do Rust */
  emit: typeof mockEmit;
  /** Volta o dataset e o catálogo ao estado inicial */
  reset: () => void;
}

declare global {
  interface Window {
    __octapusMock?: MockControl;
  }
}

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
  // `getCurrentWindow()` (barra de título) lê a janela atual dos metadados.
  mockWindows('main');

  window.__octapusMock = {
    store: useMockStore,
    emit: mockEmit,
    reset: () => {
      resetDataset();
      mockCatalog.reset();
    },
  };

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
