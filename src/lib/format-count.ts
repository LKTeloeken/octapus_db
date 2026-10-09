const formatter = new Intl.NumberFormat('pt-BR');

/** Contagem com separador de milhar (5.002) — schemas, tabelas, resultados */
export function formatCount(value: number): string {
  return formatter.format(value);
}
