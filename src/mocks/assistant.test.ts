import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  acceptAssistantConsent,
  clearAssistantFeedback,
  getAssistantConsent,
  getAssistantMe,
  getAssistantModels,
  getAssistantStatus,
  logoutAssistant,
  redeemAssistantInvite,
  setAssistantFeedback,
} from '@/api/assistant';
import { ApiError } from '@/api/client';
import { consentPending, quotaRemaining } from '@/queries/use-assistant-me';
import { RustCommand } from '@/api/commands';
import type {
  AssistantConversation,
  AssistantConversationSummary,
  AssistantFeedbackInput,
  AssistantTurnEvent,
  AssistantTurnFinish,
} from '@/api/types/assistant.types';
import type { QueryResult } from '@/api/types/query.types';
import { resetAssistantSession } from './assistant';
import { resetAssistantConversations } from './assistant-turns';
import { useMockStore } from './mock-store';
import { route } from './router';

// O `invoke` do app passa pelo router do mock, como no `pnpm dev:mock`
vi.mock('@tauri-apps/api/core', async original => ({
  ...(await original<typeof import('@tauri-apps/api/core')>()),
  invoke: (command: string, args?: Record<string, unknown>) =>
    route(command, args),
}));

async function codeOf(promise: Promise<unknown>): Promise<string> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof ApiError) return error.code;
  }
  throw new Error('esperava um ApiError');
}

describe('assistente no mock', () => {
  beforeEach(() => {
    resetAssistantSession();
    useMockStore.setState({
      latencyMs: 0,
      failMode: 'off',
      assistantMode: 'normal',
    });
  });

  it('convite → consentimento → /me', async () => {
    expect(await getAssistantStatus()).toMatchObject({
      configured: true,
      online: true,
      loggedIn: false,
    });
    expect(await codeOf(getAssistantMe())).toBe('ASSISTANT_UNAUTHORIZED');

    const participant = await redeemAssistantInvite('CONVITE-TCC-07');
    expect(participant.label).toBe('P-07');
    expect((await getAssistantStatus()).loggedIn).toBe(true);

    // Logado, mas o termo ainda não foi aceito
    const before = await getAssistantMe();
    expect(consentPending(before)).toBe(true);

    const term = await getAssistantConsent();
    expect(await codeOf(acceptAssistantConsent('versao-velha'))).toBe(
      'ASSISTANT_CONFLICT',
    );
    await acceptAssistantConsent(term.version);

    const me = await getAssistantMe();
    expect(consentPending(me)).toBe(false);
    expect(me.consent.acceptedVersion).toBe(term.version);
    expect(me.modelSelection).toBe('free');
    expect(quotaRemaining(me)).toBe(
      me.quota.limitTokens - me.quota.usedTokens - me.quota.reservedTokens,
    );
    expect((await getAssistantModels()).length).toBeGreaterThan(0);
  });

  it('convite curto ou revogado é recusado', async () => {
    expect(await codeOf(redeemAssistantInvite('curto'))).toBe(
      'ASSISTANT_INVALID_REQUEST',
    );
    expect(await codeOf(redeemAssistantInvite('REVOGADO-01'))).toBe(
      'ASSISTANT_UNAUTHORIZED',
    );
    expect((await getAssistantStatus()).loggedIn).toBe(false);
  });

  it('sair esquece o login, mesmo sem a API', async () => {
    await redeemAssistantInvite('CONVITE-TCC-07');
    useMockStore.setState({ assistantMode: 'offline' });
    await logoutAssistant();
    useMockStore.setState({ assistantMode: 'normal' });
    expect((await getAssistantStatus()).loggedIn).toBe(false);
  });

  it('sem URL: status desligado e comandos ASSISTANT_NOT_CONFIGURED', async () => {
    useMockStore.setState({ assistantMode: 'not-configured' });
    expect(await getAssistantStatus()).toMatchObject({
      configured: false,
      online: false,
    });
    expect(await codeOf(redeemAssistantInvite('CONVITE-TCC-07'))).toBe(
      'ASSISTANT_NOT_CONFIGURED',
    );
  });

  it('API fora: status offline e comandos ASSISTANT_UNAVAILABLE', async () => {
    useMockStore.setState({ assistantMode: 'offline' });
    expect(await getAssistantStatus()).toMatchObject({
      configured: true,
      online: false,
    });
    expect(await codeOf(getAssistantMe())).toBe('ASSISTANT_UNAVAILABLE');
  });

  describe('turnos', () => {
    beforeEach(async () => {
      resetAssistantConversations();
      await redeemAssistantInvite('CONVITE-TCC-07');
      await acceptAssistantConsent((await getAssistantConsent()).version);
    });

    // O Channel de verdade precisa do Tauri: o mock só chama `onmessage`
    async function ask(
      conversationId: string,
      question: string,
      id = 'turn-1',
    ) {
      const events: AssistantTurnEvent[] = [];
      const finish = (await route(RustCommand.AssistantTurnAsk, {
        serverId: 1,
        database: 'loja',
        conversationId,
        question,
        clientTurnId: id,
        onEvent: {
          onmessage: (event: AssistantTurnEvent) => events.push(event),
        },
      })) as AssistantTurnFinish;
      return { finish, events, types: events.map(event => event.type) };
    }

    /** O evento `context` de um turno com menções */
    async function contextOf(
      serverId: number,
      database: string,
      mentions: { schema: string | null; table: string | null }[],
      schemas: string[] | null,
    ) {
      const { id } = (await route(RustCommand.AssistantConversationCreate, {
        serverId,
        modelId: null,
        title: null,
      })) as AssistantConversationSummary;
      const events: AssistantTurnEvent[] = [];
      await route(RustCommand.AssistantTurnAsk, {
        serverId,
        database,
        conversationId: id,
        question: 'quantos registros?',
        clientTurnId: `turn-${serverId}-${database}`,
        schemas,
        mentions,
        pastMentions: null,
        onEvent: {
          onmessage: (event: AssistantTurnEvent) => events.push(event),
        },
      });
      return events.find(event => event.type === 'context');
    }

    it('menções: a inexistente vira aviso; o @schema grande, recorte', async () => {
      const context = await contextOf(
        1,
        'saas',
        [
          { schema: 'tenant_00042', table: 'orders' },
          { schema: 'TENANT_00042', table: null },
          { schema: 'tenant_99999', table: 'orders' },
          { schema: 'public', table: 'ghost' },
        ],
        ['tenant_00042', 'public'],
      );
      expect(context).toMatchObject({
        truncated: true,
        missingMentions: [
          { schema: 'tenant_99999', table: 'orders' },
          { schema: 'public', table: 'ghost' },
        ],
        shortenedMentions: [{ schema: 'tenant_00042', table: null }],
      });
    });

    it('menções sem schema (SQLite): pelo nome, sem diferenciar caixa', async () => {
      const context = await contextOf(
        3,
        'main',
        [
          { schema: null, table: 'NOTAS' },
          { schema: null, table: 'fantasma' },
        ],
        null,
      );
      expect(context).toMatchObject({
        missingMentions: [{ schema: null, table: 'fantasma' }],
        shortenedMentions: [],
      });
    });

    async function conversation() {
      return (await route(RustCommand.AssistantConversationCreate, {
        serverId: 1,
        modelId: null,
        title: null,
      })) as AssistantConversationSummary;
    }

    it('pergunta → eventos na ordem → turno salvo na conversa', async () => {
      const { id } = await conversation();
      const { finish, types } = await ask(
        id,
        'quantos itens existem no estoque?',
      );

      expect(finish.status).toBe('answered');
      expect(types.slice(0, 3)).toEqual([
        'turnOpened',
        'context',
        'attemptStarted',
      ]);
      expect(types).toContain('output');
      expect(types.at(-1)).toBe('plan');

      const saved = (await route(RustCommand.AssistantConversation, {
        conversationId: id,
      })) as AssistantConversation;
      expect(saved.turns).toHaveLength(1);
      expect(saved.turns[0].attempts[0].output?.kind).toBe('query');
    });

    it('o delta é o JSON da resposta em pedaços, como na API', async () => {
      const { id } = await conversation();
      const { finish, events } = await ask(id, 'quantos itens no estoque?');
      const raw = events
        .map(event => (event.type === 'delta' ? event.text : ''))
        .join('');
      expect(JSON.parse(raw)).toEqual(finish.output);
    });

    it('correção depois do erro, código e esclarecimento', async () => {
      const { id } = await conversation();

      const corrected = await ask(id, 'total com erro de coluna, por favor');
      expect(corrected.types).toContain('stageFailed');
      expect(
        corrected.types.filter(type => type === 'attemptStarted'),
      ).toHaveLength(2);

      const code = await ask(
        id,
        'apagar o primeiro cliente da lista',
        'turn-2',
      );
      expect(code.finish.status).toBe('code_delivered');
      expect(code.types).not.toContain('result');

      const unclear = await ask(id, 'e aí?', 'turn-3');
      expect(unclear.finish.status).toBe('clarification');
    });

    it('resposta em texto e recusa fora do tema: replied, sem executar', async () => {
      const { id } = await conversation();

      const concept = await ask(id, 'o que é um índice composto?');
      expect(concept.finish.status).toBe('replied');
      expect(concept.finish.output?.kind).toBe('answer');
      expect(concept.finish.output?.query).toBeNull();
      expect(concept.finish.output?.explanation).toContain('```sql');
      expect(concept.types).not.toContain('plan');
      expect(concept.types).not.toContain('result');

      const offTopic = await ask(
        id,
        'quem ganhou o jogo de futebol?',
        'turn-2',
      );
      expect(offTopic.finish.status).toBe('replied');
      expect(offTopic.finish.output?.explanation).toMatch(/só consigo ajudar/);
    });

    it('pedido de estrutura: busca, avisa os não encontrados e responde', async () => {
      const { id } = await conversation();
      const { finish, events, types } = await ask(
        id,
        'quantos itens existem? confira a estrutura antes',
      );

      expect(finish.status).toBe('answered');
      expect(finish.output?.kind).toBe('query');
      expect(types.filter(type => type === 'attemptStarted')).toHaveLength(2);
      // O pedido não vira `output`: só o evento das contagens
      expect(types.filter(type => type === 'output')).toHaveLength(1);
      const lookup = events.find(event => event.type === 'structureLookup');
      expect(lookup).toMatchObject({
        requested: 2,
        found: 1,
        missing: ['fantasma'],
      });

      const saved = (await route(RustCommand.AssistantConversation, {
        conversationId: id,
      })) as AssistantConversation;
      const kinds = saved.turns[0].attempts.map(item => item.output?.kind);
      expect(kinds).toEqual(['structure_request', 'query']);
    });

    it('pergunta ao usuário: espera a resposta e segue com ela', async () => {
      const { id } = await conversation();
      // Antes de perguntar, não há o que responder
      expect(
        await route(RustCommand.AssistantTurnReply, {
          clientTurnId: 'turn-q',
          reply: 'cedo',
        }),
      ).toBe(false);

      const running = ask(id, 'quantas vendas tivemos em outubro?', 'turn-q');
      // Espera a pergunta chegar
      for (let i = 0; i < 100; i += 1) {
        await new Promise(resolve => setTimeout(resolve, 20));
        const answered = await route(RustCommand.AssistantTurnReply, {
          clientTurnId: 'turn-q',
          reply: '  Data do pagamento ',
        });
        if (answered) break;
      }
      const { finish, events } = await running;

      expect(finish.status).toBe('answered');
      expect(finish.output?.assumptions.at(-1)).toMatch(/data do pagamento/);
      expect(events.find(event => event.type === 'question')).toMatchObject({
        choices: ['Data do pedido', 'Data do pagamento'],
      });
      expect(events.find(event => event.type === 'userReplied')).toMatchObject({
        reply: 'Data do pagamento',
      });

      const saved = (await route(RustCommand.AssistantConversation, {
        conversationId: id,
      })) as AssistantConversation;
      const [question, answer] = saved.turns[0].attempts;
      expect(question.output?.kind).toBe('ask_user');
      expect(question.userReply).toBeNull();
      expect(answer.userReply).toBe('Data do pagamento');
    });

    it('cancelar enquanto espera a resposta encerra o turno', async () => {
      const { id } = await conversation();
      const running = ask(id, 'quantos clientes ativos temos?', 'turn-c');
      await new Promise(resolve => setTimeout(resolve, 1_200));
      expect(
        await route(RustCommand.AssistantTurnCancel, {
          clientTurnId: 'turn-c',
        }),
      ).toBe(true);
      expect((await running).finish.status).toBe('cancelled');
    });

    it('cancelar no meio encerra o turno como cancelado', async () => {
      const { id } = await conversation();
      const running = ask(id, 'quantos pedidos existem no total?', 'turn-x');
      await new Promise(resolve => setTimeout(resolve, 30));
      expect(
        await route(RustCommand.AssistantTurnCancel, {
          clientTurnId: 'turn-x',
        }),
      ).toBe(true);
      expect((await running).finish.status).toBe('cancelled');
      expect(
        await route(RustCommand.AssistantTurnCancel, {
          clientTurnId: 'turn-x',
        }),
      ).toBe(false);
    });

    it('execução manual só aceita leitura', async () => {
      await expect(
        route(RustCommand.AssistantExecute, {
          query: 'SELECT count(*) FROM t',
          page: {},
        }),
      ).resolves.toMatchObject({ rowCount: 1 });
      // Listagem: páginas pelo offset (C-09)
      const second = (await route(RustCommand.AssistantExecute, {
        query: 'SELECT id FROM t',
        page: { offset: 400, limit: 200 },
      })) as QueryResult;
      expect(second.rows).toHaveLength(50);
      expect(second.hasMore).toBe(false);
      expect(second.editableInfo).toBeNull();
      await expect(
        route(RustCommand.AssistantExecute, {
          query: 'DELETE FROM t',
          page: {},
        }),
      ).rejects.toMatchObject({ code: 'GUARD_REJECTED' });
    });

    it('Mongo: operação do contrato, código do shell e guard na execução', async () => {
      const mongo = (await route(RustCommand.AssistantConversationCreate, {
        serverId: 2,
        modelId: null,
        title: null,
      })) as AssistantConversationSummary;
      expect(mongo.paradigm).toBe('mongodb');

      const events: AssistantTurnEvent[] = [];
      const finish = (await route(RustCommand.AssistantTurnAsk, {
        serverId: 2,
        database: 'catalogo',
        conversationId: mongo.id,
        question: 'quantos produtos existem no catálogo?',
        clientTurnId: 'turn-mongo',
        onEvent: {
          onmessage: (event: AssistantTurnEvent) => events.push(event),
        },
      })) as AssistantTurnFinish;
      expect(finish.status).toBe('answered');
      expect(finish.output?.language).toBe('mongo');
      expect(JSON.parse(finish.output?.query ?? '')).toMatchObject({
        collection: 'produtos',
        operation: 'countDocuments',
      });

      const code = (await route(RustCommand.AssistantTurnAsk, {
        serverId: 2,
        database: 'catalogo',
        conversationId: mongo.id,
        question: 'apagar o primeiro produto da lista',
        clientTurnId: 'turn-mongo-2',
      })) as AssistantTurnFinish;
      expect(code.output?.query).toMatch(/^db\.produtos\.deleteOne/);

      const page = { offset: 0, limit: 200 };
      const listed = (await route(RustCommand.AssistantExecute, {
        serverId: 2,
        query: JSON.stringify({ collection: 'produtos', operation: 'find' }),
        page,
      })) as QueryResult;
      expect(listed.columns[0]).toMatchObject({
        name: '_id',
        typeName: 'objectId',
      });
      for (const query of [
        JSON.stringify({
          collection: 'produtos',
          operation: 'aggregate',
          pipeline: [{ $out: 'x' }],
        }),
        JSON.stringify({ collection: 'produtos', operation: 'deleteMany' }),
        'db.produtos.find({})',
      ]) {
        await expect(
          route(RustCommand.AssistantExecute, { serverId: 2, query, page }),
        ).rejects.toMatchObject({ code: 'GUARD_REJECTED' });
      }
    });

    it('Redis: plano de passos, total da contagem, código e guard na execução', async () => {
      const redis = (await route(RustCommand.AssistantConversationCreate, {
        serverId: 4,
        modelId: null,
        title: null,
      })) as AssistantConversationSummary;
      expect(redis.paradigm).toBe('redis');

      const events: AssistantTurnEvent[] = [];
      const finish = (await route(RustCommand.AssistantTurnAsk, {
        serverId: 4,
        database: '0',
        conversationId: redis.id,
        question: 'quantos usuários existem?',
        clientTurnId: 'turn-redis',
        onEvent: {
          onmessage: (event: AssistantTurnEvent) => events.push(event),
        },
      })) as AssistantTurnFinish;
      expect(finish.status).toBe('answered');
      expect(finish.output?.language).toBe('redis_plan');
      expect(JSON.parse(finish.output?.query ?? '')).toMatchObject({
        steps: [
          { command: 'SCAN', args: ['0', 'MATCH', 'user:*', 'TYPE', 'hash'] },
        ],
        output: 'keys',
      });
      expect(events).toContainEqual(
        expect.objectContaining({
          type: 'plan',
          plan: { estimatedRows: null, estimatedCost: null, plan: 'SCAN' },
        }),
      );

      const code = (await route(RustCommand.AssistantTurnAsk, {
        serverId: 4,
        database: '0',
        conversationId: redis.id,
        question: 'apagar o primeiro usuário da lista',
        clientTurnId: 'turn-redis-2',
      })) as AssistantTurnFinish;
      expect(code.output?.query).toBe('DEL user:1');

      const page = { offset: 0, limit: 200 };
      const counted = (await route(RustCommand.AssistantExecute, {
        serverId: 4,
        query: finish.output?.query,
        page,
      })) as QueryResult;
      expect(counted.totalCount).toBe(1284);
      expect(counted.columns.map(column => column.name)).toEqual(['key']);
      const step = (
        command: string,
        args: string[],
        foreach: string | null = null,
      ) => ({
        id: 'a',
        command,
        args,
        foreach,
      });
      for (const query of [
        JSON.stringify({ steps: [step('KEYS', ['*'])], output: 'a' }),
        JSON.stringify({
          steps: [step('EVAL', ['return 1', '0'])],
          output: 'a',
        }),
        JSON.stringify({ steps: [step('DEL', ['user:1'])], output: 'a' }),
        'SCAN 0 MATCH user:*',
      ]) {
        await expect(
          route(RustCommand.AssistantExecute, { serverId: 4, query, page }),
        ).rejects.toMatchObject({ code: 'GUARD_REJECTED' });
      }
    });

    it('avaliação: grava, troca, remove e volta na conversa', async () => {
      const { id } = await conversation();
      const { finish } = await ask(id, 'quantos itens existem no estoque?');
      const turnId = finish.turnId as string;
      const read = async () =>
        (
          (await route(RustCommand.AssistantConversation, {
            conversationId: id,
          })) as AssistantConversation
        ).turns[0].feedback;

      await setAssistantFeedback(turnId, {
        rating: 'down',
        reasons: ['slow', 'slow', 'wrong_query'],
        comment: '  faltou o filtro  ',
        attemptId: finish.attemptId,
      });
      const down = await read();
      expect(down).toMatchObject({
        rating: 'down',
        reasons: ['slow', 'wrong_query'],
        comment: 'faltou o filtro',
      });

      // Trocar para 👍 larga os motivos e mantém a primeira data
      await setAssistantFeedback(turnId, {
        rating: 'up',
        reasons: ['slow'],
        comment: null,
        attemptId: finish.attemptId,
      });
      expect(await read()).toMatchObject({
        rating: 'up',
        reasons: [],
        createdAt: down?.createdAt,
      });

      await clearAssistantFeedback(turnId);
      expect(await read()).toBeNull();
      // Remover de novo não é erro
      await clearAssistantFeedback(turnId);
    });

    it('avaliação recusada: comentário longo, tentativa alheia, turno inexistente', async () => {
      const { id } = await conversation();
      const { finish } = await ask(id, 'quantos itens existem no estoque?');
      const turnId = finish.turnId as string;
      const up = {
        rating: 'up',
        reasons: [],
        comment: null,
      } satisfies Partial<AssistantFeedbackInput>;

      expect(
        await codeOf(
          setAssistantFeedback(turnId, {
            ...up,
            comment: 'a'.repeat(2001),
            attemptId: null,
          }),
        ),
      ).toBe('ASSISTANT_INVALID_REQUEST');
      expect(
        await codeOf(
          setAssistantFeedback(turnId, { ...up, attemptId: 'outra' }),
        ),
      ).toBe('ASSISTANT_INVALID_REQUEST');
      expect(
        await codeOf(
          setAssistantFeedback('00000000-0000-4000-8000-ffffffffffff', {
            ...up,
            attemptId: null,
          }),
        ),
      ).toBe('ASSISTANT_NOT_FOUND');
    });

    it('sem termo aceito, a conversa não abre', async () => {
      resetAssistantSession();
      await redeemAssistantInvite('CONVITE-TCC-07');
      await expect(conversation()).rejects.toMatchObject({
        code: 'ASSISTANT_CONSENT_REQUIRED',
      });
    });
  });
});
