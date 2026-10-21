import { describe, expect, it } from 'vitest';
import {
  conversationMentions,
  conversationSchemas,
  MENTION_HIGHLIGHT,
  mentionDraft,
  mentionLabel,
  parseMentions,
  quoteIdent,
  unquoteIdent,
} from './mentions';

describe('parseMentions', () => {
  it('lê schema e tabela no Postgres', () => {
    const text = 'faça uma relação das tabelas @public.user e @public.recibos';
    expect(parseMentions(text, false)).toEqual([
      { from: 29, to: 41, schema: 'public', table: 'user' },
      { from: 44, to: 59, schema: 'public', table: 'recibos' },
    ]);
  });

  it('@nome sozinho é o schema inteiro', () => {
    expect(parseMentions('tudo de @tenant_0042', false)).toEqual([
      { from: 8, to: 20, schema: 'tenant_0042', table: null },
    ]);
  });

  it('aceita nomes entre aspas', () => {
    const [mention] = parseMentions('@"Vendas"."Pedidos ""25"""?', false);
    expect(mention.schema).toBe('Vendas');
    expect(mention.table).toBe('Pedidos "25"');
    expect(mention.to).toBe(26);
  });

  it('não confunde e-mail nem pontuação no fim', () => {
    expect(parseMentions('mande para ana@empresa.com', false)).toEqual([]);
    expect(parseMentions('(@public.users), ok.', false)).toEqual([
      { from: 1, to: 14, schema: 'public', table: 'users' },
    ]);
    expect(parseMentions('veja @public.users.', false)[0].table).toBe('users');
  });

  it('sem schema, @tabela direto', () => {
    expect(parseMentions('linhas de @orders e @"Itens"', true)).toEqual([
      { from: 10, to: 17, schema: null, table: 'orders' },
      { from: 20, to: 28, schema: null, table: 'Itens' },
    ]);
  });
});

describe('quoteIdent / unquoteIdent', () => {
  it('ida e volta', () => {
    for (const name of ['users', 'Pedidos 2025', 'a"b', 'ção_1']) {
      expect(unquoteIdent(quoteIdent(name))).toBe(name);
    }
    expect(quoteIdent('users')).toBe('users');
    expect(quoteIdent('Pedidos 2025')).toBe('"Pedidos 2025"');
  });
});

describe('conversationSchemas', () => {
  it('menções da pergunta primeiro (a primeira vai no search_path)', () => {
    expect(
      conversationSchemas(
        'junte @vendas.pedidos com @public.users',
        [],
        null,
        false,
      ),
    ).toEqual(['vendas', 'public']);
  });

  it('depois as das perguntas anteriores, da mais recente, e o padrão', () => {
    expect(
      conversationSchemas(
        'agora só os de 2025',
        ['quantos em @t1.orders?', 'e em @t2.orders?'],
        'app',
        false,
      ),
    ).toEqual(['t2', 't1', 'app']);
  });

  it('sem menção nem padrão, public', () => {
    expect(conversationSchemas('quantos usuários?', [], null, false)).toEqual([
      'public',
    ]);
  });

  it('não repete', () => {
    expect(conversationSchemas('@a.x @a.y @b', ['@b.z'], 'a', false)).toEqual([
      'a',
      'b',
    ]);
  });

  it('sem schema, nada (o backend usa o schema sem nome)', () => {
    expect(conversationSchemas('@orders', [], null, true)).toEqual([]);
  });
});

describe('conversationMentions', () => {
  it('as da pergunta atual são obrigatórias; as anteriores, sem repetir', () => {
    expect(
      conversationMentions(
        'total de @vendas.pedidos por @tenant_7 e @vendas.pedidos',
        ['@public.users', 'e @vendas.pedidos? e @"Vendas"."Itens 2025"'],
        false,
      ),
    ).toEqual({
      mentions: [
        { schema: 'vendas', table: 'pedidos' },
        { schema: 'tenant_7', table: null },
      ],
      // Da mais recente para a mais antiga, sem as que já estão na atual
      pastMentions: [
        { schema: 'Vendas', table: 'Itens 2025' },
        { schema: 'public', table: 'users' },
      ],
    });
  });

  it('sem schema (SQLite, Mongo): só a tabela', () => {
    expect(conversationMentions('@orders e @"Order Items"', [], true)).toEqual({
      mentions: [
        { schema: null, table: 'orders' },
        { schema: null, table: 'Order Items' },
      ],
      pastMentions: [],
    });
  });
});

describe('mentionLabel', () => {
  it('escreve como o texto da menção', () => {
    expect(mentionLabel({ schema: 'vendas', table: 'pedidos' })).toBe(
      '@vendas.pedidos',
    );
    expect(mentionLabel({ schema: 'tenant_7', table: null })).toBe('@tenant_7');
    expect(mentionLabel({ schema: null, table: 'Order Items' })).toBe(
      '@"Order Items"',
    );
  });
});

describe('mentionDraft', () => {
  it('reconhece a menção sendo digitada', () => {
    expect(mentionDraft('veja @')).toEqual({
      text: '@',
      first: '',
      second: null,
    });
    expect(mentionDraft('veja @pub')).toEqual({
      text: '@pub',
      first: 'pub',
      second: null,
    });
    expect(mentionDraft('@public.')).toEqual({
      text: '@public.',
      first: 'public',
      second: '',
    });
    expect(mentionDraft('x @"Ven das".pe')).toEqual({
      text: '@"Ven das".pe',
      first: 'Ven das',
      second: 'pe',
    });
  });

  it('fora de menção, nada', () => {
    expect(mentionDraft('ana@empresa')).toBeNull();
    expect(mentionDraft('@public.users ')).toBeNull();
    expect(mentionDraft('sem arroba')).toBeNull();
  });
});

describe('MENTION_HIGHLIGHT', () => {
  it('pega só o @nome', () => {
    const found = 'a @public.users e ana@x.com e (@t1)'.match(
      MENTION_HIGHLIGHT,
    );
    expect(found).toEqual(['@public.users', '@t1']);
  });
});
