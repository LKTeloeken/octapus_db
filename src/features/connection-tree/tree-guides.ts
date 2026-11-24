/**
 * Guias de tentáculo da árvore (DESIGN.md, Árvore): para cada linha, o que
 * desenhar em cada nível de recuo acima dela.
 *
 * - `line`: o ancestro daquele nível ainda tem irmãos abaixo — linha inteira;
 * - `tee`: a própria linha tem irmãos abaixo — linha inteira, que a liga;
 * - `end`: é o último filho — a linha desce até o meio e faz a curva;
 * - `none`: nada.
 *
 * Uma passada de trás para a frente: `hasNext[k]` diz se, depois da linha
 * atual, ainda vem uma linha de nível `k` antes de alguma de nível menor.
 */
export type GuideKind = 'none' | 'line' | 'tee' | 'end';

export function computeTreeGuides(levels: readonly number[]): GuideKind[][] {
  const out: GuideKind[][] = new Array(levels.length);
  const hasNext: boolean[] = [];
  for (let index = levels.length - 1; index >= 0; index--) {
    const level = levels[index];
    const guides: GuideKind[] = [];
    for (let k = 1; k <= level; k++) {
      const next = hasNext[k] ?? false;
      if (k === level) guides.push(next ? 'tee' : 'end');
      else guides.push(next ? 'line' : 'none');
    }
    out[index] = guides;
    hasNext[level] = true;
    hasNext.length = level + 1;
  }
  return out;
}
