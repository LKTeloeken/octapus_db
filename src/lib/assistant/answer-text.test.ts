import { describe, expect, it } from 'vitest';
import { splitAnswer, splitInline } from './answer-text';

describe('splitAnswer', () => {
  it('separa parágrafos e mantém as quebras de linha dentro deles', () => {
    expect(
      splitAnswer('Primeira linha\nsegunda linha\n\n\nOutro parágrafo'),
    ).toEqual([
      {
        type: 'paragraph',
        spans: [{ code: false, text: 'Primeira linha\nsegunda linha' }],
      },
      { type: 'paragraph', spans: [{ code: false, text: 'Outro parágrafo' }] },
    ]);
  });

  it('bloco cercado vira código, com a linguagem e sem as cercas', () => {
    const text =
      'Use um índice:\n```sql\nCREATE INDEX i\n  ON t (a);\n```\nPronto.';
    expect(splitAnswer(text)).toEqual([
      { type: 'paragraph', spans: [{ code: false, text: 'Use um índice:' }] },
      { type: 'code', language: 'sql', code: 'CREATE INDEX i\n  ON t (a);' },
      { type: 'paragraph', spans: [{ code: false, text: 'Pronto.' }] },
    ]);
  });

  it('cerca sem fechar (resposta chegando) é código até o fim', () => {
    expect(splitAnswer('Assim:\n```\nSELECT 1')).toEqual([
      { type: 'paragraph', spans: [{ code: false, text: 'Assim:' }] },
      { type: 'code', language: null, code: 'SELECT 1' },
    ]);
  });

  it('HTML fica como texto', () => {
    const [block] = splitAnswer('<script>alert(1)</script> <b>x</b>');
    expect(block).toEqual({
      type: 'paragraph',
      spans: [{ code: false, text: '<script>alert(1)</script> <b>x</b>' }],
    });
  });

  it('texto vazio não rende bloco', () => {
    expect(splitAnswer('  \n\n')).toEqual([]);
  });
});

describe('splitInline', () => {
  it('crase simples vira código em linha', () => {
    expect(splitInline('Use `JOIN` em `orders.customer_id`.')).toEqual([
      { code: false, text: 'Use ' },
      { code: true, text: 'JOIN' },
      { code: false, text: ' em ' },
      { code: true, text: 'orders.customer_id' },
      { code: false, text: '.' },
    ]);
    expect(splitInline('sem crase de fechar `x')).toEqual([
      { code: false, text: 'sem crase de fechar `x' },
    ]);
  });
});
