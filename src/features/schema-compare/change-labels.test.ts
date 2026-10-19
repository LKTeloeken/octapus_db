import { describe, expect, it } from 'vitest';
import { describeChange } from './change-labels';

const spec = (
  dataType: string,
  nullable = true,
  defaultValue: string | null = null,
) => ({
  dataType,
  nullable,
  defaultValue,
});

describe('describeChange', () => {
  it('coluna que só um dos schemas tem mostra a definição', () => {
    expect(
      describeChange({
        column: 'discount',
        expected: null,
        actual: spec('numeric', false, '0'),
      }),
    ).toEqual({
      tone: 'onlyOther',
      label: 'só no outro',
      detail: 'numeric NOT NULL DEFAULT 0',
    });
    expect(
      describeChange({
        column: 'status',
        expected: spec('text'),
        actual: null,
      }),
    ).toEqual({ tone: 'onlyBase', label: 'só na base', detail: 'text' });
  });

  it('definição diferente lista só o que mudou, da base para o outro', () => {
    expect(
      describeChange({
        column: 'status',
        expected: spec('text', true, "'novo'"),
        actual: spec('character varying(20)', false, "'novo'"),
      }).detail,
    ).toBe('text → character varying(20) · aceita nulo → NOT NULL');
    expect(
      describeChange({
        column: 'status',
        expected: spec('text', true, "'novo'"),
        actual: spec('text'),
      }).detail,
    ).toBe("default 'novo' → nenhum");
  });
});
