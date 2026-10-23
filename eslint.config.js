import js from '@eslint/js';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

/**
 * Lint do front. Erro reprova o `pnpm lint` (e a CI); aviso é dívida a pagar
 * aos poucos. Os tipos ficam com o `tsc` (`pnpm type-check`) — aqui é só o
 * que o compilador não vê: regras dos hooks e acessibilidade.
 */
export default tseslint.config(
  {
    ignores: [
      'dist',
      'src-tauri',
      'node_modules',
      'perf',
      'playwright-report',
      'test-results',
    ],
  },
  {
    files: [
      'src/**/*.{ts,tsx}',
      'e2e/**/*.ts',
      '*.config.{js,ts}',
      'scripts/**/*.mjs',
    ],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      ecmaVersion: 2022,
      globals: { ...globals.browser, ...globals.node },
    },
    rules: {
      // `_` na frente: parâmetro exigido pela assinatura, mas sem uso
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
    },
  },
  {
    files: ['src/**/*.tsx', 'src/**/use-*.ts'],
    plugins: { 'react-hooks': reactHooks, 'jsx-a11y': jsxA11y },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // Por enquanto aviso: há dependências omitidas de propósito
      'react-hooks/exhaustive-deps': 'warn',
      // Acessibilidade entra como aviso; vira erro quando o código estiver limpo
      // (o nível vem como 'error' ou ['error', opções]: mantém as opções)
      ...Object.fromEntries(
        Object.entries(jsxA11y.flatConfigs.recommended.rules)
          .map(([rule, level]) => [rule, [level].flat()])
          .filter(([, [severity]]) => severity !== 'off')
          .map(([rule, [, ...options]]) => [rule, ['warn', ...options]]),
      ),
      // autoFocus só existe em campo que o próprio usuário acabou de abrir
      // (diálogo, editor de célula, paleta) — nunca ao carregar a tela
      'jsx-a11y/no-autofocus': 'off',
      // Checkbox e Switch do Radix são o controle dentro do <label>
      'jsx-a11y/label-has-associated-control': [
        'warn',
        { controlComponents: ['Checkbox', 'Switch'], depth: 3 },
      ],
    },
  },
);
