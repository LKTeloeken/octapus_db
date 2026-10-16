import { describe, expect, it } from 'vitest';
import {
  CODE_RULES,
  CSS_RULES,
  findViolations,
} from './check-design-tokens.mjs';

const rulesIn = (text, rules = CODE_RULES) =>
  findViolations(text, rules).map(v => `${v.rule}:${v.match}`);

describe('check-design-tokens', () => {
  it('aceita os tokens do Ink', () => {
    expect(
      rulesIn(
        "cn('bg-surface-1 text-fg-muted border-line hover:bg-hover ring-ring bg-iris-soft text-danger transition-colors')",
      ),
    ).toEqual([]);
  });

  it('pega paleta crua do Tailwind, com e sem opacidade', () => {
    expect(
      rulesIn("'bg-red-500 hover:text-white/60 border-slate-200'"),
    ).toEqual([
      'raw-palette:bg-red-500',
      'raw-palette:text-white/60',
      'raw-palette:border-slate-200',
    ]);
  });

  it('pega aliases do shadcn que não existem mais', () => {
    expect(
      rulesIn(
        "'bg-background text-muted-foreground bg-primary' + 'var(--card)'",
      ),
    ).toEqual([
      'shadcn-alias:bg-background',
      'shadcn-alias:text-muted-foreground',
      'shadcn-alias:bg-primary',
      'shadcn-alias:var(--card)',
    ]);
  });

  it('pega cor literal, inclusive dentro de valor arbitrário', () => {
    expect(
      rulesIn(
        "'bg-[#1e1e2e] shadow-[inset_0_0_0_1px_oklch(1_0_0/35%)]' + 'rgba(0,0,0,.5)'",
      ),
    ).toEqual([
      'hex-color:#1e1e2e',
      'color-function:oklch(',
      'color-function:rgba(',
    ]);
  });

  it('não confunde âncora, entidade ou id com hex', () => {
    expect(rulesIn("href='#results' '&#123;' id='cafe-bar'")).toEqual([]);
  });

  it('pega transition-all no TSX e transition: all no CSS', () => {
    expect(rulesIn("'transition-all duration-180'")).toEqual([
      'transition-all:transition-all',
    ]);
    expect(rulesIn('.a { transition: all 120ms; }', CSS_RULES)).toEqual([
      'transition-all:transition: all',
    ]);
  });

  it('respeita o design-tokens-ignore na linha ou na de cima', () => {
    expect(
      rulesIn(
        [
          '// design-tokens-ignore: branco sobre Iris',
          "'text-white'",
          "'bg-white' // design-tokens-ignore: idem",
          "'text-black'",
        ].join('\n'),
      ),
    ).toEqual(['raw-palette:text-black']);
  });
});
