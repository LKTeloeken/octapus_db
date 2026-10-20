/**
 * As cenas do polvo animado. Fora do motor (sem GSAP), para quem só escolhe
 * a cena não puxar o chunk.
 */
export type OctapusSceneId =
  /** Base: respira, ergue um tentáculo e solta bolhas de pensamento */
  | 'thinking'
  /** Lupa: lendo a estrutura do banco */
  | 'magnifier'
  /** Notebook ao lado: escrevendo a consulta */
  | 'computer'
  /** Prancheta ao lado: conferindo a consulta */
  | 'clipboard'
  /** Quatro tabelas viram uma (JOIN): executando */
  | 'tables'
  /** Ampulheta: esperando a vez (rate limit) */
  | 'hourglass'
  /** Coçando a cabeça, uma vez: falhou */
  | 'scratch'
  /** Pulo com brilhos, uma vez: respondeu */
  | 'celebrate';
