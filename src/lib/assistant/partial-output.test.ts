import { describe, expect, it } from 'vitest';
import type { ModelOutput } from '@/api/types/assistant.types';
import { extractPartialOutput } from './partial-output';

const FULL: ModelOutput = {
  kind: 'query',
  language: 'sql',
  query:
    'SELECT "nome", count(*)\nFROM "public"."users"\nWHERE note = \'a "b"\'',
  explanation: 'Conta os usuários — por nome.\tTab e \\ barra.',
  assumptions: ['"ativo" quer dizer status = 1', 'Sem filtro de data'],
  clarificationQuestion: null,
  structureRequest: null,
};

describe('extractPartialOutput', () => {
  it('lê o JSON completo igual ao parse', () => {
    const partial = extractPartialOutput(JSON.stringify(FULL));
    expect(partial).toEqual({
      kind: FULL.kind,
      query: FULL.query,
      explanation: FULL.explanation,
      assumptions: FULL.assumptions,
      clarificationQuestion: null,
    });
  });

  it('nunca quebra e só cresce, cortando em qualquer ponto', () => {
    const raw = JSON.stringify(FULL, null, 2);
    let previousQuery = '';
    let previousExplanation = '';
    for (let cut = 0; cut <= raw.length; cut += 1) {
      const partial = extractPartialOutput(raw.slice(0, cut));
      const query = partial.query ?? '';
      const explanation = partial.explanation ?? '';
      // Prefixos do valor final: o texto na tela nunca "volta"
      expect(FULL.query?.startsWith(query)).toBe(true);
      expect(FULL.explanation.startsWith(explanation)).toBe(true);
      expect(query.length).toBeGreaterThanOrEqual(previousQuery.length);
      expect(explanation.length).toBeGreaterThanOrEqual(
        previousExplanation.length,
      );
      previousQuery = query;
      previousExplanation = explanation;
    }
  });

  it('mostra a string que ainda não fechou', () => {
    const partial = extractPartialOutput(
      '{"kind":"query","language":"sql","query":"SELECT * FR',
    );
    expect(partial.kind).toBe('query');
    expect(partial.query).toBe('SELECT * FR');
    expect(partial.explanation).toBeNull();
  });

  it('só aceita o kind fechado e do contrato', () => {
    expect(extractPartialOutput('{"kind":"que').kind).toBeNull();
    expect(
      extractPartialOutput('{"kind":"outro","query":"x"}').kind,
    ).toBeNull();
    expect(extractPartialOutput('{"kind":"code"').kind).toBe('code');
    expect(extractPartialOutput('{"kind":"answer","expl').kind).toBe('answer');
    expect(extractPartialOutput('{"kind":"structure_request"').kind).toBe(
      'structure_request',
    );
  });

  it('descarta escape cortado no meio', () => {
    expect(extractPartialOutput('{"explanation":"a\\').explanation).toBe('a');
    expect(extractPartialOutput('{"explanation":"a\\u00').explanation).toBe(
      'a',
    );
    expect(extractPartialOutput('{"explanation":"a\\u00e9b').explanation).toBe(
      'aéb',
    );
    expect(extractPartialOutput('{"explanation":"l1\\nl2"}').explanation).toBe(
      'l1\nl2',
    );
  });

  it('aceita campos fora de ordem e pula os desconhecidos', () => {
    const partial = extractPartialOutput(
      '{"extra":{"a":[1,"}"]},"explanation":"E","n":12,"ok":true,"query":"Q"}',
    );
    expect(partial.explanation).toBe('E');
    expect(partial.query).toBe('Q');
  });

  it('premissas chegam uma a uma, a última incompleta', () => {
    expect(
      extractPartialOutput('{"assumptions":["um","do').assumptions,
    ).toEqual(['um', 'do']);
    expect(extractPartialOutput('{"assumptions":["um",').assumptions).toEqual([
      'um',
    ]);
    expect(extractPartialOutput('{"assumptions":[').assumptions).toEqual([]);
  });

  it('ignora a cerca de código antes do objeto', () => {
    const partial = extractPartialOutput('```json\n{"query":"SELECT 1"');
    expect(partial.query).toBe('SELECT 1');
    expect(extractPartialOutput('```jso').query).toBeNull();
  });

  it('valor de tipo inesperado não vira texto (query objeto, nulo)', () => {
    const partial = extractPartialOutput(
      '{"query":{"collection":"c","operation":"find"},"clarificationQuestion":null,"explanation":"x"}',
    );
    expect(partial.query).toBeNull();
    expect(partial.clarificationQuestion).toBeNull();
    expect(partial.explanation).toBe('x');
  });

  it('texto que não é objeto não rende nada', () => {
    expect(extractPartialOutput('SELECT 1').query).toBeNull();
    expect(extractPartialOutput('').explanation).toBeNull();
    expect(extractPartialOutput('   ').assumptions).toEqual([]);
  });
});
