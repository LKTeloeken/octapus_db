import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  composite,
  contrastRatio,
  parseOklch,
  parseThemes,
  resolveToken,
  type Rgba,
  type Theme,
} from './contrast';

/**
 * Contraste dos tokens do Ink nos dois temas (DESIGN.md §2 e checklist do
 * §12): texto 4,5:1 sobre as camadas em que aparece, inclusive as elevadas e
 * o vidro; ícones de banco 3:1. Token novo de texto ou camada nova entra aqui.
 */
const css = readFileSync(new URL('./globals.css', import.meta.url), 'utf8');
const themes = parseThemes(css);
const THEMES: Theme[] = ['dark', 'light'];

const TEXT_AA = 4.5;
const ICON_AA = 3;

const WHITE = parseOklch('oklch(1 0 0)');

/** As camadas em que cada grupo de texto aparece, já compostas */
function layers(theme: Theme) {
  const t = (name: string) => resolveToken(themes[theme], name);
  const s1 = t('surface-1');
  const s3 = t('surface-3');
  const glass = composite(t('bg'), t('glass'));
  return {
    t,
    panels: {
      bg: t('bg'),
      'surface-1': s1,
      'surface-2': t('surface-2'),
      'surface-3': s3,
      'surface-1 + hover': composite(s1, t('hover')),
      'surface-1 + active': composite(s1, t('active')),
      'surface-3 + active': composite(s3, t('active')),
      glass,
      'glass + active': composite(glass, t('active')),
      'surface-1 + control': composite(s1, t('control')),
      'surface-1 + field': composite(s1, t('field')),
    },
    grid: {
      'surface-1': s1,
      'surface-1 + hover': composite(s1, t('hover')),
      'surface-1 + iris-soft': composite(s1, t('iris-soft')),
      'surface-1 + success-soft': composite(s1, t('success-soft')),
      'surface-1 + warning-soft': composite(s1, t('warning-soft')),
    },
    editor: {
      'surface-1': s1,
      'surface-1 + hover': composite(s1, t('hover')),
      'surface-1 + iris-soft': composite(s1, t('iris-soft')),
      'surface-1 + active': composite(s1, t('active')),
    },
  };
}

interface Pair {
  theme: Theme;
  text: string;
  on: string;
  ratio: number;
  min: number;
}

function pairs(): Pair[] {
  const out: Pair[] = [];
  for (const theme of THEMES) {
    const { t, panels, grid, editor } = layers(theme);
    const add = (text: string, fg: Rgba, on: string, bg: Rgba, min: number) =>
      out.push({
        theme,
        text,
        on,
        ratio: contrastRatio(composite(bg, fg), bg),
        min,
      });
    const each = (
      texts: string[],
      surfaces: Record<string, Rgba>,
      min: number,
    ) => {
      for (const text of texts) {
        for (const [on, bg] of Object.entries(surfaces)) {
          add(`--${text}`, t(text), on, bg, min);
        }
      }
    };

    // §2.1–2.3: texto da interface sobre todas as camadas
    each(
      [
        'fg',
        'fg-muted',
        'fg-subtle',
        'iris-text',
        'success',
        'warning',
        'danger',
        'info',
      ],
      panels,
      TEXT_AA,
    );
    // §2.4/§2.6: cores do tipo na grade, também nas linhas pendentes e selecionadas
    each(
      [
        'fg',
        'fg-muted',
        'fg-subtle',
        'data-number',
        'data-date',
        'data-bool',
        'data-json',
      ],
      grid,
      TEXT_AA,
    );
    add(
      '--fg-subtle',
      t('fg-subtle'),
      'surface-1 + danger-soft',
      composite(t('surface-1'), t('danger-soft')),
      TEXT_AA,
    );
    // §2.7: sintaxe no editor (linha ativa, seleção, ocorrências da seleção)
    each(
      ['syn-keyword', 'syn-function', 'syn-string', 'syn-number', 'syn-type'],
      editor,
      TEXT_AA,
    );
    // §2.5: ícones dos bancos (nunca texto corrido)
    each(['db-postgres', 'db-mongo', 'db-redis', 'db-sqlite'], panels, ICON_AA);

    // §2.3: texto de badge sobre o próprio -soft; item destrutivo do menu
    for (const semantic of ['success', 'warning', 'danger', 'info']) {
      add(
        `--${semantic}`,
        t(semantic),
        `surface-1 + ${semantic}-soft`,
        composite(t('surface-1'), t(`${semantic}-soft`)),
        TEXT_AA,
      );
    }
    add(
      '--danger',
      t('danger'),
      'surface-3 + danger-soft',
      composite(t('surface-3'), t('danger-soft')),
      TEXT_AA,
    );

    // Assistente: avisos (recorte, código não executado, falha) em --fg sobre
    // o -soft, ícone no sólido; menção (fundo --active) no campo e na pergunta
    for (const semantic of ['info', 'warning', 'danger']) {
      const notice = composite(t('surface-1'), t(`${semantic}-soft`));
      add('--fg', t('fg'), `surface-1 + ${semantic}-soft`, notice, TEXT_AA);
      add(
        `--${semantic}`,
        t(semantic),
        `surface-1 + ${semantic}-soft`,
        notice,
        ICON_AA,
      );
    }
    add(
      '--fg',
      t('fg'),
      'surface-1 + field + active',
      composite(t('surface-1'), t('field'), t('active')),
      TEXT_AA,
    );
    add(
      '--fg',
      t('fg'),
      'surface-1 + active + hover',
      composite(t('surface-1'), t('active'), t('hover')),
      TEXT_AA,
    );

    // §2.2: branco sobre os preenchimentos (botão primário, hover do destrutivo)
    add('branco', WHITE, '--iris', t('iris'), TEXT_AA);
    add('branco', WHITE, '--iris-hover', t('iris-hover'), TEXT_AA);
    add('branco', WHITE, '--danger-solid', t('danger-solid'), TEXT_AA);

    // §2.4: trecho casado da busca, também numa linha selecionada
    for (const match of ['match-soft', 'match-strong']) {
      add(
        '--fg',
        t('fg'),
        `surface-1 + ${match}`,
        composite(t('surface-1'), t(match)),
        TEXT_AA,
      );
      add(
        '--fg',
        t('fg'),
        `iris-soft + ${match}`,
        composite(t('surface-1'), t('iris-soft'), t(match)),
        TEXT_AA,
      );
    }
  }
  return out;
}

const label = (p: Pair) => `${p.theme}: ${p.text} sobre ${p.on}`;

/**
 * Pares abaixo do mínimo aceitos temporariamente, com o motivo — hoje nenhum.
 * O teste falha se um deles melhorar sem sair daqui, ou se aparecer outro.
 */
const KNOWN_FAILURES = new Set<string>([]);

describe('contraste dos tokens (DESIGN.md)', () => {
  const all = pairs();

  it('o conversor bate com os números do DESIGN.md', () => {
    const fgOnPanel = (theme: Theme) =>
      contrastRatio(
        resolveToken(themes[theme], 'fg'),
        resolveToken(themes[theme], 'surface-1'),
      );
    expect(fgOnPanel('dark')).toBeCloseTo(15.6, 1);
    expect(fgOnPanel('light')).toBeCloseTo(17.1, 1);
    expect(contrastRatio(WHITE, resolveToken(themes.dark, 'iris'))).toBeCloseTo(
      5.1,
      1,
    );
    expect(
      contrastRatio(WHITE, resolveToken(themes.light, 'iris')),
    ).toBeCloseTo(5.9, 1);
  });

  it('todo par passa o mínimo (fora os conhecidos)', () => {
    const failing = all
      .filter(p => p.ratio < p.min && !KNOWN_FAILURES.has(label(p)))
      .map(p => `${label(p)}: ${p.ratio.toFixed(2)} < ${p.min}`);
    expect(failing).toEqual([]);
  });

  it('a lista de falhas conhecidas está em dia', () => {
    const stillFailing = new Set(all.filter(p => p.ratio < p.min).map(label));
    const fixed = Array.from(KNOWN_FAILURES).filter(
      known => !stillFailing.has(known),
    );
    expect(fixed).toEqual([]);
  });

  it('todo token existe nos dois temas', () => {
    // Movimento é igual nos dois temas e só mora no :root
    const keys = (theme: Theme) =>
      Object.keys(themes[theme])
        .filter(name => !name.startsWith('duration-'))
        .sort();
    expect(keys('dark')).toEqual(keys('light'));
  });
});
