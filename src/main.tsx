import { createRoot } from 'react-dom/client';
import { StrictMode, createElement } from 'react';
import App from '@/app';
import { disableNativeSpellcheck } from '@/lib/disable-native-spellcheck';

disableNativeSpellcheck();

// Get the root element
const container = document.getElementById('root');

// Ensure the container exists
if (!container) {
  throw new Error('Root element not found in the DOM');
}

/**
 * Em `pnpm dev:mock` o backend Rust não existe: o IPC simulado precisa estar
 * instalado antes do primeiro render, senão as queries iniciais já falham.
 * Em produção `VITE_MOCK` é `undefined`, o `if` vira código morto e o
 * `import()` some do bundle.
 */
async function bootstrap() {
  if (import.meta.env.VITE_MOCK === 'true') {
    const { installMocks } = await import('@/mocks/install');
    installMocks();
  }

  // Create root using the new React 19 API
  const root = createRoot(container!);

  // Render the application
  root.render(createElement(StrictMode, null, createElement(App, null)));
}

void bootstrap();
