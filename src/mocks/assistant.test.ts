import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  acceptAssistantConsent,
  getAssistantConsent,
  getAssistantMe,
  getAssistantModels,
  getAssistantStatus,
  logoutAssistant,
  redeemAssistantInvite,
} from '@/api/assistant';
import { ApiError } from '@/api/client';
import { consentPending, quotaRemaining } from '@/queries/use-assistant-me';
import { RustCommand } from '@/api/commands';
import type {
  AssistantConversation,
  AssistantConversationSummary,
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

    it('sem termo aceito, a conversa não abre', async () => {
      resetAssistantSession();
      await redeemAssistantInvite('CONVITE-TCC-07');
      await expect(conversation()).rejects.toMatchObject({
        code: 'ASSISTANT_CONSENT_REQUIRED',
      });
    });
  });
});
