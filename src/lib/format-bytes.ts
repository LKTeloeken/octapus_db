const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const;

/**
 * Tamanho em bytes legível na maior unidade que caiba (base 1024): `512 B`,
 * `8 KB`, `1.4 MB`, `412 MB`, `2.1 GB`. Abaixo de 10 mostra uma casa decimal
 * (sem o `.0`); daí pra cima, inteiro — o bastante para a árvore.
 */
export function formatBytes(bytes: number): string {
  let value = Math.max(0, bytes);
  let unit = 0;

  // 1023.5 e não 1024: senão 1023.8 KB arredondaria para "1024 KB"
  while (value >= 1023.5 && unit < UNITS.length - 1) {
    value /= 1024;
    unit++;
  }

  const text =
    unit === 0 || value >= 10
      ? String(Math.round(value))
      : value.toFixed(1).replace(/\.0$/, '');

  return `${text} ${UNITS[unit]}`;
}
