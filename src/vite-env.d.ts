/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 'true' apenas em `pnpm dev:mock` — liga o backend simulado (src/mocks/) */
  readonly VITE_MOCK?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
