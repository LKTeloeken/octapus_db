import { describe, expect, it } from 'vitest';
import { phaseLabel, structureSummary } from './turn-messages';

describe('structureSummary', () => {
  it('ao vivo: soma as rodadas e nomeia os que não existem', () => {
    expect(
      structureSummary([
        { requested: 2, found: 1, missing: ['ghost'], limitReached: false },
        { requested: 1, found: 1, missing: [], limitReached: false },
      ]),
    ).toBe(
      'O assistente consultou a estrutura de 3 nomes: 2 encontrados. Não existem no banco: ghost.',
    );
  });

  it('o teto do turno aparece', () => {
    expect(
      structureSummary([
        { requested: 1, found: 1, missing: [], limitReached: false },
        { requested: 2, found: 0, missing: [], limitReached: true },
      ]),
    ).toBe(
      'O assistente consultou a estrutura de 1 nome: 1 encontrado. Chegou ao limite de consultas à estrutura nesta pergunta.',
    );
  });

  it('histórico: só os nomes pedidos', () => {
    expect(
      structureSummary([
        { requested: 3, found: null, missing: null, limitReached: false },
      ]),
    ).toBe('O assistente consultou a estrutura do banco (3 nomes pedidos).');
  });
});

describe('phaseLabel', () => {
  it('pedido de estrutura', () => {
    expect(phaseLabel('structure', true, false)).toBe(
      'Consultando a estrutura…',
    );
  });
});
