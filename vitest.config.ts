import { configDefaults, defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config';

// Testes do front. Os de desempenho (`*.perf.test.ts[x]`, em src/perf) só entram
// com OCTAPUS_PERF=1: são lentos, medem com GC exposto e gravam em
// perf/catalog/results.
const perf = process.env.OCTAPUS_PERF === '1';
const perfFiles = 'src/**/*.perf.test.{ts,tsx}';

export default defineConfig(env =>
  mergeConfig(viteConfig(env), {
    test: {
      environment: 'node',
      include: perf ? [perfFiles] : ['src/**/*.test.{ts,tsx}'],
      exclude: perf
        ? configDefaults.exclude
        : [...configDefaults.exclude, perfFiles],
      passWithNoTests: true,
      pool: 'forks',
      poolOptions: { forks: { singleFork: true, execArgv: ['--expose-gc'] } },
      testTimeout: perf ? 15 * 60_000 : 5_000,
    },
  }),
);
