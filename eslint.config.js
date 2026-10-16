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
  { ignores: ['dist', 'src-tauri', 'node_modules', 'perf'] },
  {
    files: ['src/**/*.{ts,tsx}', '*.config.{js,ts}', 'scripts/**/*.mjs'],
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
      ...Object.fromEntries(
        Object.entries(jsxA11y.flatConfigs.recommended.rules)
          .filter(([, level]) => level !== 'off')
          .map(([rule]) => [rule, 'warn']),
      ),
    },
  },
);
