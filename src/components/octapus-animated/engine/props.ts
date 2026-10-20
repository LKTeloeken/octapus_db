import { svgEl } from './rig';

/**
 * Objetos das cenas. Tudo em `currentColor` (cheio, traço ou opacidade): o
 * tema vem da cor do texto, sem cor crua. Traços grossos (≥ 26 unidades do
 * viewBox ≈ 1 px a 32 px), para ler pequeno. Cada objeto é desenhado em volta
 * do próprio ponto de pega (0,0).
 */

/** Margem do recorte em volta de cada objeto (fresta contra o corpo) */
const HALO = 16;

/**
 * Pincel: `cut = false` desenha o objeto; `cut = true` desenha a silhueta dele
 * em preto, com a margem, para a máscara do corpo (ver `rig.cutFront`).
 */
type Attrs = Record<string, string | number>;

const brush = (cut: boolean) => ({
  stroke: (width: number): Attrs => ({
    fill: 'none',
    stroke: cut ? 'black' : 'currentColor',
    'stroke-width': cut ? width + HALO * 2 : width,
    'stroke-linecap': 'round',
    'stroke-linejoin': 'round',
  }),
  solid: (): Attrs =>
    cut
      ? {
          fill: 'black',
          stroke: 'black',
          'stroke-width': HALO * 2,
          'stroke-linejoin': 'round',
        }
      : { fill: 'currentColor' },
  /** Preenchimento translúcido: no recorte, cheio (abre o fundo) ou nada */
  tint: (opacity: number, cutsThrough = true): Attrs =>
    cut
      ? cutsThrough
        ? { fill: 'black', stroke: 'black', 'stroke-width': HALO * 2 }
        : { fill: 'none' }
      : { fill: 'currentColor', opacity },
});

const stroke = brush(false).stroke;

/** Lupa: pega no fim do cabo; a lente fica a (-207, -246) da pega */
export function magnifier(parent: Element, cut = false) {
  const b = brush(cut);
  const g = svgEl('g', {}, parent);
  const tilt = svgEl('g', { transform: 'rotate(-40) scale(1.4)' }, g);
  svgEl(
    'rect',
    { x: -18, y: -150, width: 36, height: 130, rx: 18, ...b.solid() },
    tilt,
  );
  const lensGroup = svgEl('g', { transform: 'translate(0 -230)' }, tilt);
  // O vidro não recorta: dentro da lente o rosto continua (o olho cresce)
  const glass = svgEl('circle', { r: 78, ...b.tint(0.14, false) }, lensGroup);
  svgEl('circle', { r: 78, ...b.stroke(30) }, lensGroup);
  const glint = svgEl(
    'path',
    {
      d: 'M -40 -28 A 50 50 0 0 1 -6 -50',
      ...b.stroke(14),
      opacity: cut ? 0 : 0.6,
    },
    lensGroup,
  );
  return { g, glass, glint, lensGroup };
}

/**
 * Notebook visto de trás, em três quartos: a tampa (com o logo vazado) e a
 * base por baixo. A tela fica virada para o polvo, ao lado; acima da tampa,
 * linhas de código sobem enquanto ele digita. Origem no meio da base.
 */
export function laptopBack(parent: Element, cut = false) {
  const b = brush(cut);
  const g = svgEl('g', {}, parent);
  svgEl(
    'path',
    {
      d: 'M -150 -22 L 146 -16 Q 158 -16 158 -4 L 158 -2 Q 158 8 146 8 L -156 2 Q -168 2 -166 -10 Q -164 -22 -150 -22 Z',
      ...b.solid(),
    },
    g,
  );
  // A tampa: o lado direito (mais perto) um pouco maior — perspectiva
  const lid =
    'M -122 -32 L 126 -26 Q 146 -26 146 -46 L 150 -206 Q 150 -228 128 -226 L -104 -196 Q -122 -194 -123 -176 L -138 -50 Q -140 -32 -122 -32 Z';
  const logo = 'M 32 -118 A 24 24 0 1 0 -16 -118 A 24 24 0 1 0 32 -118 Z';
  svgEl(
    'path',
    { d: cut ? lid : `${lid} ${logo}`, 'fill-rule': 'evenodd', ...b.solid() },
    g,
  );
  const lines = cut
    ? []
    : [
        { x: -10, w: 110 },
        { x: 10, w: 140 },
        { x: -10, w: 80 },
      ].map(({ x, w }) =>
        svgEl(
          'rect',
          {
            x,
            y: -11,
            width: w,
            height: 22,
            rx: 11,
            fill: 'currentColor',
            opacity: 0.7,
          },
          g,
        ),
      );
  return { g, lines };
}

/** Tabelinha: cabeçalho cheio e linhas; pega no centro */
export function miniTable(parent: Element, columns = 2, cut = false) {
  const b = brush(cut);
  const g = svgEl('g', {}, parent);
  const width = 70 * columns + 40;
  const x = -width / 2;
  svgEl('rect', { x, y: -80, width, height: 160, rx: 20, ...b.tint(0.14) }, g);
  svgEl('rect', { x, y: -80, width, height: 160, rx: 20, ...b.stroke(22) }, g);
  if (cut) return g;
  svgEl(
    'rect',
    { x, y: -80, width, height: 44, rx: 20, fill: 'currentColor' },
    g,
  );
  for (const dy of [10, 50]) {
    svgEl(
      'line',
      { x1: x + 26, y1: dy, x2: -x - 26, y2: dy, ...stroke(16), opacity: 0.55 },
      g,
    );
  }
  for (let c = 1; c < columns; c += 1) {
    const cx = x + (width / columns) * c;
    svgEl(
      'line',
      { x1: cx, y1: -36, x2: cx, y2: 70, ...stroke(12), opacity: 0.4 },
      g,
    );
  }
  return g;
}

/** Prancheta com três itens; os vistos se desenham pelo `stroke-dashoffset` */
export function clipboard(parent: Element, cut = false) {
  const b = brush(cut);
  const g = svgEl('g', {}, parent);
  svgEl(
    'rect',
    { x: -150, y: -200, width: 300, height: 380, rx: 30, ...b.tint(0.14) },
    g,
  );
  svgEl(
    'rect',
    { x: -150, y: -200, width: 300, height: 380, rx: 30, ...b.stroke(26) },
    g,
  );
  svgEl(
    'rect',
    { x: -60, y: -232, width: 120, height: 60, rx: 18, ...b.solid() },
    g,
  );
  const checks: SVGPathElement[] = [];
  const bars: SVGLineElement[] = [];
  if (cut) return { g, checks, bars };
  [-110, -10, 90].forEach(y => {
    const check = svgEl(
      'path',
      {
        d: `M -110 ${y} l 26 26 l 44 -50`,
        ...stroke(22),
        pathLength: 1,
        'stroke-dasharray': 1,
        'stroke-dashoffset': 1,
      },
      g,
    );
    checks.push(check);
    bars.push(
      svgEl(
        'line',
        { x1: -10, y1: y, x2: 100, y2: y, ...stroke(20), opacity: 0.55 },
        g,
      ),
    );
  });
  return { g, checks, bars };
}

/** Ampulheta: a areia de cima esvazia, a de baixo enche; gira no meio (0,0) */
export function hourglass(parent: Element, cut = false) {
  const b = brush(cut);
  const g = svgEl('g', {}, parent);
  svgEl('line', { x1: -95, y1: -170, x2: 95, y2: -170, ...b.stroke(30) }, g);
  svgEl('line', { x1: -95, y1: 170, x2: 95, y2: 170, ...b.stroke(30) }, g);
  svgEl(
    'path',
    {
      d: 'M -70 -160 C -70 -60 -12 -30 -12 0 C -12 30 -70 60 -70 160 L 70 160 C 70 60 12 30 12 0 C 12 -30 70 -60 70 -160 Z',
      ...(cut ? b.tint(1) : b.stroke(20)),
    },
    g,
  );
  const top = svgEl(
    'path',
    {
      d: 'M -52 -120 L 52 -120 C 40 -60 10 -30 0 -10 C -10 -30 -40 -60 -52 -120 Z',
      fill: cut ? 'none' : 'currentColor',
      opacity: 0.6,
    },
    g,
  );
  const bottom = svgEl(
    'path',
    {
      d: 'M -56 150 C -50 110 -20 90 0 86 C 20 90 50 110 56 150 Z',
      fill: cut ? 'none' : 'currentColor',
      opacity: 0.6,
    },
    g,
  );
  const stream = svgEl(
    'line',
    { x1: 0, y1: 0, x2: 0, y2: 120, ...stroke(8), opacity: cut ? 0 : 0.6 },
    g,
  );
  return { g, top, bottom, stream };
}

/** Gota de suor */
export function drop(parent: Element) {
  return svgEl(
    'path',
    {
      d: 'M 0 -46 C 22 -14 34 6 34 22 A 34 34 0 0 1 -34 22 C -34 6 -22 -14 0 -46 Z',
      fill: 'currentColor',
      opacity: 0.55,
    },
    parent,
  );
}

/** Brilho de quatro pontas */
export function sparkle(parent: Element, size = 60) {
  const s = size;
  return svgEl(
    'path',
    {
      d: `M 0 ${-s} Q ${s * 0.15} ${-s * 0.15} ${s} 0 Q ${s * 0.15} ${s * 0.15} 0 ${s} Q ${-s * 0.15} ${s * 0.15} ${-s} 0 Q ${-s * 0.15} ${-s * 0.15} 0 ${-s} Z`,
      fill: 'currentColor',
    },
    parent,
  );
}

/** Bolhas de pensamento (três círculos crescentes) */
export function thoughtDots(parent: Element) {
  const g = svgEl('g', {}, parent);
  const dots = [
    { cx: 0, cy: 0, r: 22 },
    { cx: 50, cy: -60, r: 32 },
    { cx: 120, cy: -140, r: 44 },
  ].map(d => svgEl('circle', { ...d, fill: 'currentColor', opacity: 0.5 }, g));
  return { g, dots };
}
