#!/usr/bin/env node
/**
 * Verifica no código do front as regras de cor e movimento do DESIGN.md
 * (§9 e checklist do §12). Sai com código 1 se achar violação.
 *
 * - cor crua: paleta do Tailwind (`bg-red-500`, `text-white/60`), `#hex`,
 *   `rgb()`/`hsl()`/`oklch()`… em TS/TSX — só tokens do Ink;
 * - aliases do shadcn que não existem mais (`bg-background`, `text-muted-foreground`…);
 * - `transition-all` (e `transition: all` no CSS).
 *
 * Exceção pontual: um comentário `design-tokens-ignore: <motivo>` na própria
 * linha ou na de cima. O `globals.css` é onde os tokens nascem: lá só vale a
 * regra do `transition: all`.
 *
 * Uso: `node scripts/check-design-tokens.mjs` (ou `pnpm lint:tokens`).
 */
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const PALETTE =
  'slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose';

const COLOR_UTILITY =
  'bg|text|border(?:-[trblxyse])?|ring(?:-offset)?|outline|divide|fill|stroke|from|via|to|decoration|accent|caret|placeholder|shadow|inset-shadow|inset-ring';

const SHADCN_ALIAS =
  'background|foreground|primary(?:-foreground)?|secondary(?:-foreground)?|muted(?:-foreground)?|accent(?:-foreground)?|destructive(?:-foreground)?|card(?:-foreground)?|popover(?:-foreground)?|input|sidebar(?:-[a-z-]+)?|chart-\\d';

/** Regras para arquivos TS/TSX */
export const CODE_RULES = [
  {
    id: 'raw-palette',
    message: 'cor da paleta do Tailwind — use um token do Ink (DESIGN.md §9)',
    pattern: new RegExp(
      `(?<![a-zA-Z0-9-])(?:${COLOR_UTILITY})-(?:(?:${PALETTE})-\\d{2,3}|white|black)(?:\\/\\d+)?(?![\\w-])`,
      'g',
    ),
  },
  {
    id: 'shadcn-alias',
    message:
      'alias do shadcn que não gera CSS — traduza para o Ink (DESIGN.md §9)',
    pattern: new RegExp(
      `(?<![a-zA-Z0-9-])(?:${COLOR_UTILITY})-(?:${SHADCN_ALIAS})(?![\\w-])|var\\(--(?:${SHADCN_ALIAS})\\)`,
      'g',
    ),
  },
  {
    id: 'hex-color',
    message: 'cor em hex — use um token do Ink ou leia a variável CSS',
    pattern:
      /(?<![a-zA-Z0-9&])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?![\w-])/g,
  },
  {
    id: 'color-function',
    message: 'cor literal (rgb/hsl/oklch…) — use um token do Ink',
    pattern:
      /(?<![a-zA-Z0-9-])(?:rgba?|hsla?|oklch|oklab|lab|lch|hwb|color-mix)\(/g,
  },
  {
    id: 'transition-all',
    message:
      'anime só transform/opacity (DESIGN.md §6) — nada de transition-all',
    pattern: /(?<![a-zA-Z0-9-])transition-all(?![\w-])/g,
  },
];

/** Regras para o CSS (onde os tokens são definidos) */
export const CSS_RULES = [
  {
    id: 'transition-all',
    message:
      'anime só transform/opacity (DESIGN.md §6) — nada de transition: all',
    pattern: /transition(?:-property)?\s*:\s*all\b/g,
  },
];

const IGNORE_MARKER = 'design-tokens-ignore';

/** A linha é só um comentário com a marca (vale para a linha de baixo) */
const isIgnoreComment = line =>
  line !== undefined &&
  /^\s*(?:\/\/|\/\*|\*|\{\/\*)/.test(line) &&
  line.includes(IGNORE_MARKER);

/** Violações num arquivo: `{ rule, message, line, column, match }` */
export function findViolations(text, rules) {
  const lines = text.split('\n');
  const found = [];
  lines.forEach((line, index) => {
    if (line.includes(IGNORE_MARKER) || isIgnoreComment(lines[index - 1])) {
      return;
    }
    for (const rule of rules) {
      for (const match of line.matchAll(rule.pattern)) {
        found.push({
          rule: rule.id,
          message: rule.message,
          line: index + 1,
          column: match.index + 1,
          match: match[0],
        });
      }
    }
  });
  return found;
}

/** Fora da verificação: dados de mock e testes podem ter cor como dado */
const SKIP = [/^src\/mocks\//, /\.test\.tsx?$/];

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

function main() {
  const root = join(fileURLToPath(import.meta.url), '..', '..');
  const files = walk(join(root, 'src'))
    .map(path => relative(root, path).split('\\').join('/'))
    .filter(path => !SKIP.some(skip => skip.test(path)));

  const violations = files.flatMap(path => {
    const rules = /\.tsx?$/.test(path)
      ? CODE_RULES
      : path.endsWith('.css')
        ? CSS_RULES
        : null;
    if (!rules) return [];
    const text = readFileSync(join(root, path), 'utf8');
    return findViolations(text, rules).map(v => ({ ...v, path }));
  });

  for (const v of violations) {
    console.log(
      `${v.path}:${v.line}:${v.column}  ${v.rule}  ${v.match}  — ${v.message}`,
    );
  }
  if (violations.length > 0) {
    console.log(`\n✖ ${violations.length} violação(ões) do DESIGN.md`);
    process.exit(1);
  }
  console.log('✔ tokens do DESIGN.md: nenhuma violação');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
