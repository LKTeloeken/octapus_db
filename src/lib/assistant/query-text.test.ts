import { describe, expect, it } from 'vitest';
import {
  editorText,
  formatQueryText,
  mongoShellCommand,
  relationNouns,
} from './query-text';

const op = (fields: Record<string, unknown>) =>
  JSON.stringify({ collection: 'orders', ...fields });

/** Sem quebras de linha, para comparar o comando inteiro */
const flat = (text: string | null) => text?.replace(/\s+/g, '') ?? null;

describe('formatQueryText', () => {
  it('indenta a operação do Mongo e deixa o resto como veio', () => {
    const text = op({ operation: 'find', filter: { status: 'paid' } });
    expect(formatQueryText('mongo', text)).toBe(
      JSON.stringify(JSON.parse(text), null, 2),
    );
    expect(formatQueryText('sql', 'SELECT 1')).toBe('SELECT 1');
    // Código do Mongo (shell) e JSON ainda chegando passam como estão
    expect(formatQueryText('mongo', 'db.orders.drop()')).toBe(
      'db.orders.drop()',
    );
    expect(formatQueryText('mongo', '{"collection":"or')).toBe(
      '{"collection":"or',
    );
  });
});

describe('mongoShellCommand', () => {
  it('converte cada operação no comando do editor', () => {
    expect(
      flat(
        mongoShellCommand(
          op({ operation: 'find', filter: { a: 1 }, projection: { a: 1 } }),
        ),
      ),
    ).toBe('db.orders.find({"a":1},{"a":1})');
    expect(flat(mongoShellCommand(op({ operation: 'find' })))).toBe(
      'db.orders.find({})',
    );
    expect(
      flat(
        mongoShellCommand(
          op({ operation: 'aggregate', pipeline: [{ $count: 'n' }] }),
        ),
      ),
    ).toBe('db.orders.aggregate([{"$count":"n"}])');
    expect(flat(mongoShellCommand(op({ operation: 'countDocuments' })))).toBe(
      'db.orders.countDocuments({})',
    );
    expect(
      flat(
        mongoShellCommand(
          op({ operation: 'distinct', field: 'status', filter: { a: 1 } }),
        ),
      ),
    ).toBe('db.orders.distinct("status",{"a":1})');
  });

  it('find com ordem, salto ou limite vira aggregate na ordem do servidor', () => {
    const text = op({
      operation: 'find',
      filter: { status: 'paid' },
      projection: { total: 1 },
      sort: { total: -1 },
      skip: 5,
      limit: 10,
    });
    expect(flat(mongoShellCommand(text))).toBe(
      'db.orders.aggregate([{"$match":{"status":"paid"}},{"$sort":{"total":-1}},' +
        '{"$skip":5},{"$limit":10},{"$project":{"total":1}}])',
    );
  });

  it('o que não é operação do contrato fica de fora', () => {
    expect(mongoShellCommand('db.orders.find({})')).toBeNull();
    expect(mongoShellCommand('{"operation":"find"}')).toBeNull();
    expect(mongoShellCommand(op({ operation: 'deleteMany' }))).toBeNull();
  });
});

describe('editorText', () => {
  it('só o Mongo muda; código do shell passa como está', () => {
    expect(editorText('sql', 'SELECT 1')).toBe('SELECT 1');
    expect(editorText('mongo', 'db.orders.deleteMany({})')).toBe(
      'db.orders.deleteMany({})',
    );
    expect(flat(editorText('mongo', op({ operation: 'countDocuments' })))).toBe(
      'db.orders.countDocuments({})',
    );
  });
});

describe('relationNouns', () => {
  it('coleções no Mongo, tabelas no resto', () => {
    expect(relationNouns('mongo').many).toBe('coleções');
    expect(relationNouns('sql').one).toBe('tabela');
    expect(relationNouns(null).many).toBe('tabelas');
  });
});
