import { describe, expect, it } from 'vitest';
import { EditorState } from '@codemirror/state';
import { ensureSyntaxTree, syntaxTree } from '@codemirror/language';
import { postgresSupport } from './sql-language';

/** Nome do nó mais interno na posição do trecho `marker` (primeira ocorrência) */
function nodeAt(doc: string, marker: string, offset = 1): string {
  const state = EditorState.create({ doc, extensions: [postgresSupport()] });
  ensureSyntaxTree(state, doc.length, 5_000);
  const pos = doc.indexOf(marker) + offset;
  return syntaxTree(state).resolveInner(pos, 1).name;
}

const FUNCTION = `CREATE FUNCTION f() RETURNS int AS $$
BEGIN
  SELECT count(*) FROM pedidos WHERE id > 10;
  RETURN 1; -- fim
END;
$$ LANGUAGE plpgsql;`;

describe('PostgreSQL com corpo $$ analisado como SQL', () => {
  it('reconhece keywords, números e comentários dentro de $$', () => {
    expect(nodeAt(FUNCTION, 'SELECT count')).toBe('Keyword');
    expect(nodeAt(FUNCTION, 'RETURN 1')).toBe('Keyword');
    expect(nodeAt(FUNCTION, '10;')).toBe('Number');
    expect(nodeAt(FUNCTION, '-- fim')).toBe('LineComment');
  });

  it('mantém fora do corpo o que era', () => {
    expect(nodeAt(FUNCTION, 'LANGUAGE plpgsql')).toBe('Keyword');
    expect(nodeAt("SELECT 'select 1'", "'select", 2)).toBe('String');
  });

  it('aceita tag nomeada e corpo ainda sem fechamento', () => {
    const tagged = 'DO $body$ BEGIN PERFORM 1; END $body$;';
    expect(nodeAt(tagged, 'BEGIN')).toBe('Keyword');

    const open = 'CREATE FUNCTION g() RETURNS void AS $$\nBEGIN\n  SELECT 1';
    expect(nodeAt(open, 'SELECT 1')).toBe('Keyword');
  });
});
