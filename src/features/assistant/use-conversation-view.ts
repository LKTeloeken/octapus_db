import { useCallback, useMemo } from 'react';
import { ApiError } from '@/api/client';
import type {
  AssistantMe,
  AssistantTurnEvent,
} from '@/api/types/assistant.types';
import {
  failTurn,
  finishTurn,
  reduceTurn,
  startTurn,
} from '@/lib/assistant/turn-reducer';
import { quotaRemaining } from '@/queries/use-assistant-me';
import { useAssistantModels } from '@/queries/use-assistant-models';
import {
  useAskAssistant,
  useAssistantConversation,
  useCancelAssistantTurn,
  useCreateAssistantConversation,
} from '@/queries/use-assistant-conversations';
import { useDbCapabilities } from '@/queries/use-capabilities';
import { relationNouns } from '@/lib/assistant/query-text';
import { isFlatCatalog, useCatalogStatus } from '@/queries/use-catalog';
import { useServers } from '@/queries/use-servers';
import { useSettings } from '@/queries/use-settings';
import {
  useAssistantStore,
  type ConversationTab,
} from '@/stores/assistant-store';
import { conversationSchemas } from './mentions';
import { liveTurnView, serverTurnView, type TurnView } from './turn-view';

/** Um turno na tela, com o que a execução precisa saber dele */
export interface ConversationTurn {
  view: TurnView;
  /** Os `schemas` do turno; o primeiro é o `search_path` da execução */
  schemas: string[];
  /** Já está na conversa lida do servidor (só então dá para avaliar) */
  inHistory: boolean;
}

/**
 * Os eventos do canal de um turno vão para o store pela aba — mesmo que a
 * pessoa troque de aba ou feche o painel no meio, nada se perde.
 */
function turnEventHandler(tabId: string) {
  return (event: AssistantTurnEvent) => {
    const store = useAssistantStore.getState();
    store.updateLive(tabId, turn => reduceTurn(turn, event, Date.now()));
    const turn = useAssistantStore.getState().live[tabId];

    if (event.type === 'plan') {
      store.patchRun(event.attemptId, { plan: event.plan });
    }
    if (event.type === 'result') {
      const attempt = turn?.attempts.find(
        candidate => candidate.attemptId === event.attemptId,
      );
      store.patchRun(event.attemptId, {
        result: event.result,
        query: attempt?.output?.query ?? null,
        userEdited: false,
        error: null,
      });
    }
    if (event.type === 'context' && turn?.turnId) {
      store.setTurnTruncated(turn.turnId, event.truncated);
    }
  };
}

const errorOf = (error: unknown) => ({
  code: error instanceof ApiError ? error.code : 'UNKNOWN',
  message: error instanceof Error ? error.message : String(error),
});

export const useConversationView = ({
  tab,
  me,
}: {
  tab: ConversationTab;
  me: AssistantMe;
}) => {
  const { data: servers } = useServers();
  const capsByType = useDbCapabilities();
  const { data: settings } = useSettings();
  const target = tab.target;
  const server = servers?.find(item => item.id === target?.serverId) ?? null;
  const caps = server ? capsByType[server.dbType] : undefined;
  const flat = isFlatCatalog(caps);
  // Em que o assistente escreve para este banco (SQL, operação Mongo…)
  const queryLanguage = caps?.queryLanguage ?? null;

  // Abrir o catálogo conecta o database e deixa as menções prontas
  const catalog = useCatalogStatus(target?.serverId, target?.database, {
    enabled: !!caps?.supportsAssistant,
  });

  const conversation = useAssistantConversation(tab.conversationId);
  const live = useAssistantStore(state => state.live[tab.id] ?? null);
  const truncatedTurns = useAssistantStore(state => state.truncatedTurns);

  const store = useAssistantStore.getState;
  const onEvent = useMemo(() => turnEventHandler(tab.id), [tab.id]);
  const create = useCreateAssistantConversation();
  const ask = useAskAssistant(onEvent);
  const cancel = useCancelAssistantTurn();

  const freeChoice = me.modelSelection === 'free' && !tab.conversationId;
  const models = useAssistantModels(freeChoice);

  const serverTurns = useMemo(
    () =>
      [...(conversation.data?.turns ?? [])].sort((a, b) => a.index - b.index),
    [conversation.data],
  );
  const previousQuestions = useMemo(
    () => serverTurns.map(turn => turn.question),
    [serverTurns],
  );

  const turns = useMemo<ConversationTurn[]>(() => {
    const list = serverTurns.map((turn, index) => ({
      // O turno desta sessão segue com a visão ao vivo: ela tem o que o
      // servidor não guarda (código do erro, mensagem do banco, recorte)
      view:
        live && live.turnId === turn.id
          ? { ...liveTurnView(live), feedback: turn.feedback }
          : serverTurnView(turn, truncatedTurns[turn.id] ?? null),
      schemas: conversationSchemas(
        turn.question,
        previousQuestions.slice(0, index),
        target?.schema ?? null,
        flat,
      ),
      inHistory: true,
    }));
    // O turno ao vivo fica até a conversa relida do servidor trazê-lo
    if (live && !serverTurns.some(turn => turn.id === live.turnId)) {
      list.push({
        view: liveTurnView(live),
        schemas: conversationSchemas(
          live.question,
          previousQuestions,
          target?.schema ?? null,
          flat,
        ),
        inHistory: false,
      });
    }
    return list;
  }, [serverTurns, previousQuestions, truncatedTurns, live, target, flat]);

  const running = !!live && live.phase !== 'finished';
  const quotaLeft = quotaRemaining(me);
  const quotaExhausted = quotaLeft <= 0;

  /** Por que ainda não dá para perguntar (ou `null`) */
  const blockedReason = !target
    ? 'Escolha um servidor e um database.'
    : caps && !caps.supportsAssistant
      ? 'O assistente ainda não atende este tipo de banco.'
      : catalog.isError
        ? 'Não consegui conectar a este database.'
        : quotaExhausted
          ? 'Sua cota de uso do assistente acabou.'
          : null;

  const send = useCallback(async () => {
    const question = tab.draft.trim();
    if (!question || !target || running || blockedReason) return;

    const clientTurnId = crypto.randomUUID();
    const schemas = conversationSchemas(
      question,
      previousQuestions,
      target.schema,
      flat,
    );
    const { setLive, setDraft, setConversation, updateLive } = store();
    setLive(
      tab.id,
      startTurn(
        clientTurnId,
        question,
        settings?.assistantAutoExecute ?? false,
      ),
    );
    setDraft(tab.id, '');
    const mine = (update: Parameters<typeof updateLive>[1]) =>
      updateLive(tab.id, turn =>
        turn.clientTurnId === clientTurnId ? update(turn) : turn,
      );

    try {
      let conversationId = tab.conversationId;
      if (!conversationId) {
        const created = await create.mutateAsync({
          serverId: target.serverId,
          modelId: freeChoice
            ? (tab.modelId ?? models.data?.[0]?.id ?? null)
            : null,
        });
        conversationId = created.id;
        setConversation(tab.id, conversationId);
      }

      const finish = await ask.mutateAsync({
        serverId: target.serverId,
        database: target.database,
        conversationId,
        question,
        clientTurnId,
        schemas: flat ? undefined : schemas,
      });
      mine(turn => finishTurn(turn, finish));
    } catch (error) {
      mine(turn => failTurn(turn, errorOf(error)));
    }
  }, [
    tab,
    target,
    running,
    blockedReason,
    previousQuestions,
    flat,
    store,
    settings?.assistantAutoExecute,
    create,
    ask,
    freeChoice,
    models.data,
  ]);

  const cancelTurn = useCallback(() => {
    if (live && live.phase !== 'finished') cancel.mutate(live.clientTurnId);
  }, [cancel, live]);

  return {
    target,
    server,
    flat,
    queryLanguage,
    nouns: relationNouns(queryLanguage),
    catalogError: catalog.isError,
    conversation: conversation.data ?? null,
    // Na 1ª pergunta a conversa acabou de nascer: o turno ao vivo já está na tela
    conversationLoading:
      !!tab.conversationId && conversation.isPending && !live,
    conversationError: conversation.error,
    reloadConversation: () => void conversation.refetch(),
    turns,
    running,
    quota: me.quota,
    quotaLeft,
    blockedReason,
    draft: tab.draft,
    setDraft: (draft: string) => store().setDraft(tab.id, draft),
    send,
    cancelTurn,
    isCancelling: cancel.isPending,
    freeChoice,
    models: models.data ?? [],
    modelId: tab.modelId ?? models.data?.[0]?.id ?? null,
    setModel: (modelId: string) => store().setModel(tab.id, modelId),
    focusRequest: useAssistantStore(state => state.focusRequest),
  };
};
