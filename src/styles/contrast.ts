/**
 * Contraste WCAG dos tokens do Ink, calculado direto do `globals.css` — usado
 * pelo teste de contraste (contrast.test.ts). OKLCH → sRGB pelas matrizes do
 * CSS Color 4; cor fora do gamut é cortada (os tokens já nascem dentro dele) e
 * a transparência é composta em sRGB, como o navegador pinta.
 */

export type Theme = 'light' | 'dark';
/** sRGB codificado (0–1) + alpha */
export type Rgba = [number, number, number, number];
export type Tokens = Record<string, string>;

/** Os tokens de `:root` (claro) e `.dark` (escuro) */
export function parseThemes(css: string): Record<Theme, Tokens> {
  return {
    light: parseBlock(css, ':root'),
    dark: parseBlock(css, '.dark'),
  };
}

function parseBlock(css: string, selector: string): Tokens {
  const start = css.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`bloco ${selector} não encontrado`);
  const end = css.indexOf('\n}', start);
  const body = css.slice(start, end);
  const tokens: Tokens = {};
  const declaration = /--([\w-]+):\s*([^;]+);/g;
  let match: RegExpExecArray | null;
  while ((match = declaration.exec(body))) {
    tokens[match[1]] = match[2].trim();
  }
  return tokens;
}

/** Valor do token, seguindo `var(--outro)` */
export function resolveToken(tokens: Tokens, name: string): Rgba {
  let value = tokens[name];
  for (let depth = 0; value?.startsWith('var(') && depth < 5; depth++) {
    value = tokens[value.slice(6, -1)];
  }
  if (!value) throw new Error(`token --${name} não existe`);
  return parseOklch(value);
}

export function parseOklch(value: string): Rgba {
  const match = value.match(
    /^oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)\s*(?:\/\s*([\d.]+)(%?))?\s*\)$/,
  );
  if (!match) throw new Error(`não é oklch: ${value}`);
  const [, l, c, h, alpha, percent] = match;
  const a = alpha === undefined ? 1 : Number(alpha) / (percent ? 100 : 1);
  return [...oklchToSrgb(Number(l), Number(c), Number(h)), a];
}

function oklchToSrgb(
  l: number,
  c: number,
  h: number,
): [number, number, number] {
  const rad = (h * Math.PI) / 180;
  const a = c * Math.cos(rad);
  const b = c * Math.sin(rad);

  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;

  const linear = [
    4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
    -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
    -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_,
  ];
  return linear.map(v => {
    const x = Math.min(1, Math.max(0, v));
    return x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055;
  }) as [number, number, number];
}

/** Pinta as camadas de baixo para cima (a primeira precisa ser opaca) */
export function composite(...layers: Rgba[]): Rgba {
  const [first, ...rest] = layers;
  if (first[3] < 1) throw new Error('a camada de baixo precisa ser opaca');
  return rest.reduce<Rgba>(
    (under, [r, g, b, a]) => [
      r * a + under[0] * (1 - a),
      g * a + under[1] * (1 - a),
      b * a + under[2] * (1 - a),
      1,
    ],
    first,
  );
}

function luminance([r, g, b]: Rgba): number {
  const lin = (v: number) =>
    v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/** Razão de contraste WCAG 2 entre duas cores opacas */
export function contrastRatio(fg: Rgba, bg: Rgba): number {
  const [hi, lo] = [luminance(fg), luminance(bg)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
