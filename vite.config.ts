import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const host = process.env.TAURI_DEV_HOST;

// Modo mock roda numa porta separada para conviver com `pnpm tauri dev` (1420)
// — e, como origin é host:port, localStorage e IndexedDB também ficam isolados.
const MOCK_PORT = 1430;

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  build: {
    outDir: "dist", // Custom output directory (default is 'dist')
    // Or use a different folder name:
    // outDir: "build",
    // outDir: "web-build",
    emptyOutDir: true, // Clean the output directory before building
  },
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: mode === "mock" ? MOCK_PORT : 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      include: ["src/**"],
      ignored: ["**/src-tauri/**"],
    },
  },
}));
