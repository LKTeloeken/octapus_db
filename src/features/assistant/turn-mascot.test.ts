import { describe, expect, it } from 'vitest';
import { endingScene, turnScene } from './turn-mascot';

describe('turnScene', () => {
  it('dá uma cena por fase do turno', () => {
    expect(turnScene('opening', false)).toBe('magnifier');
    expect(turnScene('generating', false)).toBe('thinking');
    expect(turnScene('checking', false)).toBe('clipboard');
    expect(turnScene('executing', false)).toBe('tables');
    expect(turnScene('waiting', false)).toBe('hourglass');
  });

  it('com o stream chegando, o polvo escreve no notebook', () => {
    expect(turnScene('generating', true)).toBe('computer');
  });
});

describe('endingScene', () => {
  it('comemora a resposta (consulta ou código)', () => {
    expect(endingScene('answered')).toBe('celebrate');
    expect(endingScene('code')).toBe('celebrate');
  });

  it('coça a cabeça na falha e no limite esgotado', () => {
    expect(endingScene('failed')).toBe('scratch');
    expect(endingScene('exhausted')).toBe('scratch');
  });

  it('sem cena de fim para pergunta de volta e cancelado', () => {
    expect(endingScene('clarification')).toBeNull();
    expect(endingScene('cancelled')).toBeNull();
  });
});
