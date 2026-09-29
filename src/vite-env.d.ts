/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 'true' apenas em `pnpm dev:mock` — liga o backend simulado (src/mocks/) */
  readonly VITE_MOCK?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

/** Versão do app (package.json), injetada pelo `define` do vite.config.ts */
declare const __APP_VERSION__: string;
