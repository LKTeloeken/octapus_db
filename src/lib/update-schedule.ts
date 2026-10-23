/**
 * Quando procurar versão nova com o app aberto — muita gente nunca o fecha, e
 * a checagem da partida sozinha deixaria essas pessoas para trás.
 *
 * O agendamento olha para o relógio (`lastCheckedAt`), não para um
 * `setInterval` longo: com o notebook dormindo o timer atrasa, e ao acordar a
 * conta pelo relógio já sabe que passou da hora.
 */

/** Atraso da primeira checagem, para não competir com a carga inicial do app. */
export const FIRST_CHECK_DELAY_MS = 5_000;

/** Intervalo entre checagens com o app aberto. */
export const CHECK_INTERVAL_MS = 4 * 60 * 60_000;

/**
 * Ao voltar o foco para a janela checa de novo, mas não antes disso desde a
 * última. O canal beta usa a API do GitHub (60 req/h por IP sem login).
 */
export const FOCUS_CHECK_MIN_MS = 30 * 60_000;

/** De quanto em quanto tempo o relógio é conferido. */
export const SCHEDULE_TICK_MS = 15 * 60_000;

export type UpdateStatus = 'idle' | 'available' | 'downloading' | 'ready';

interface DueOptions {
  now: number;
  /** `null` antes da primeira checagem */
  lastCheckedAt: number | null;
  status: UpdateStatus;
  /** Uma checagem já está em andamento */
  checking: boolean;
  /** Tempo mínimo desde a última checagem */
  minIntervalMs: number;
}

/**
 * Já é hora de checar? Nunca durante o download nem com o update instalado à
 * espera do reinício. Com um update disponível segue checando: pode ter saído
 * uma versão mais nova que a oferecida.
 */
export function isCheckDue({
  now,
  lastCheckedAt,
  status,
  checking,
  minIntervalMs,
}: DueOptions): boolean {
  if (checking) return false;
  if (status === 'downloading' || status === 'ready') return false;
  if (lastCheckedAt === null) return true;
  return now - lastCheckedAt >= minIntervalMs;
}

const NUMERIC = /^\d+$/;

/**
 * Compara duas versões semver (`1.1.0`, `1.1.0-beta.14`): negativo se `a` é
 * menor, positivo se maior, 0 se iguais. Uma pre-release vem antes da versão
 * final; os identificadores numéricos dela comparam como número.
 */
export function compareVersions(a: string, b: string): number {
  const [mainA, preA] = splitVersion(a);
  const [mainB, preB] = splitVersion(b);

  for (let i = 0; i < Math.max(mainA.length, mainB.length); i++) {
    const diff = (mainA[i] ?? 0) - (mainB[i] ?? 0);
    if (diff !== 0) return Math.sign(diff);
  }

  if (preA.length === 0 || preB.length === 0) {
    // A final (sem pre-release) é maior que qualquer pre-release dela
    return Math.sign(preB.length - preA.length);
  }

  for (let i = 0; i < Math.max(preA.length, preB.length); i++) {
    const partA = preA[i];
    const partB = preB[i];
    if (partA === undefined) return -1;
    if (partB === undefined) return 1;
    if (partA === partB) continue;

    const numA = NUMERIC.test(partA);
    const numB = NUMERIC.test(partB);
    if (numA && numB) return Math.sign(Number(partA) - Number(partB));
    // Identificador numérico vem antes do alfanumérico
    if (numA) return -1;
    if (numB) return 1;
    return partA < partB ? -1 : 1;
  }
  return 0;
}

function splitVersion(version: string): [number[], string[]] {
  // `v1.2.3+build`: o prefixo e os metadados de build não contam
  const clean = version.trim().replace(/^v/, '').split('+')[0];
  const dash = clean.indexOf('-');
  const main = dash === -1 ? clean : clean.slice(0, dash);
  const pre = dash === -1 ? '' : clean.slice(dash + 1);
  return [
    main.split('.').map(part => Number(part) || 0),
    pre ? pre.split('.') : [],
  ];
}
