import type { Channel } from '@tauri-apps/api/core';
import { RustCommand } from '@/api/commands';
import type {
  AssistantAttempt,
  AssistantConversation,
  AssistantConversationPage,
  AssistantConversationSummary,
  AssistantTurnEvent,
  AssistantTurnFinish,
  AssistantTurnStatus,
  ModelOutput,
} from '@/api/types/assistant.types';
import type { QueryResult } from '@/api/types/query.types';
import { requireDatabase, requireServerEntry } from './data';
import { ASSISTANT_MODELS, requireAssistantLogin } from './assistant';
import type { MockHandler } from './handlers';

/**
 * Conversas e turnos do assistente simulados (B6). O turno emite os mesmos
 * eventos do backend, com pausas, para a tela ver o andamento:
 *
 * - pergunta curta (< 12 caracteres) → esclarecimento;
 * - pede escrita ("apagar", "excluir", "delete", "insert", "cadastrar",
 *   "atualizar") → código, nunca executado;
 * - menciona "erro" → a primeira tentativa falha no preflight e a correção
 *   passa (o ciclo do C-11);
 * - o resto → consulta sobre a primeira tabela do database.
 *
 * A execução automática segue a preferência salva (`assistantAutoExecute`):
 * ligada, vem o resultado; desligada, só o plano. As conversas ficam em
 * memória (somem no reload), como o histórico que no app real mora na API.
 */

type Args = Record<string, unknown>;

const STEP_MS = 180;
const SETTINGS_STORAGE_KEY = 'octapus-mock-settings';

const conversations = new Map<string, AssistantConversation>();
/** Turnos em andamento, pelo `clientTurnId`: `true` = cancelado */
const runningTurns = new Map<string, { cancelled: boolean }>();
let sequence = 0;

const nextId = () => {
  sequence += 1;
  const hex = sequence.toString(16).padStart(12, '0');
  return `00000000-0000-4000-8000-${hex}`;
};

const now = () => new Date().toISOString();
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

/** Some com o reset do painel */
export function resetAssistantConversations() {
  conversations.clear();
  runningTurns.clear();
}

function autoExecute(): boolean {
  try {
    const saved = JSON.parse(
      globalThis.localStorage?.getItem(SETTINGS_STORAGE_KEY) ?? '{}',
    );
    return saved.assistantAutoExecute === true;
  } catch {
    return false;
  }
}

function paradigmFor(serverId: number): 'postgres' | 'sqlite' {
  const { dbType } = requireServerEntry(serverId).server;
  if (dbType === 'postgres' || dbType === 'sqlite') return dbType;
  throw `Unsupported type: the assistant does not support ${dbType} yet`;
}

function requireConversation(id: string): AssistantConversation {
  const conversation = conversations.get(id);
  if (!conversation) throw 'Assistant not_found: conversation not found';
  return conversation;
}

function outputFor(question: string, table: string): ModelOutput {
  const text = question.toLowerCase();
  if (question.trim().length < 12) {
    return {
      kind: 'clarification',
      language: null,
      query: null,
      explanation: '',
      assumptions: [],
      clarificationQuestion: `Sobre qual tabela é a pergunta? Por exemplo, "${table}".`,
    };
  }
  if (/apag|exclu|delet|insert|cadastr|atualiz/.test(text)) {
    return {
      kind: 'code',
      language: 'sql',
      query: `DELETE FROM "${table}" WHERE id = 1`,
      explanation: 'Remove a linha de id 1. O assistente não executa código.',
      assumptions: ['"id" identifica a linha'],
      clarificationQuestion: null,
    };
  }
  return {
    kind: 'query',
    language: 'sql',
    query: `SELECT count(*) AS total FROM "${table}"`,
    explanation: `Conta as linhas de ${table}.`,
    assumptions: ['Todas as linhas contam, sem filtro de data'],
    clarificationQuestion: null,
  };
}

function resultFor(): QueryResult {
  return {
    columns: [{ name: 'total', typeName: 'int8', typeOid: 20 }],
    rows: [['1284']],
    rowCount: 1,
    totalCount: null,
    hasMore: false,
    executionTimeMs: 12,
    editableInfo: null,
  };
}

const finishStatus = (output: ModelOutput): AssistantTurnStatus =>
  output.kind === 'clarification'
    ? 'clarification'
    : output.kind === 'code'
      ? 'code_delivered'
      : 'answered';

function attempt(index: number, output: ModelOutput): AssistantAttempt {
  return {
    id: nextId(),
    index,
    status: 'generated',
    output,
    usage: {
      inputTokens: 2_400,
      outputTokens: 180,
      cacheReadTokens: 0,
      cacheWriteTokens: 0,
      costUsd: 0.0094,
    },
    latencyMs: 1_800,
    outcome: null,
    createdAt: now(),
  };
}

export const assistantTurnHandlers: Record<string, MockHandler> = {
  [RustCommand.AssistantConversations]: (): AssistantConversationPage => {
    requireAssistantLogin();
    const items = Array.from(conversations.values())
      .map(({ turns: _turns, ...summary }) => summary)
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
    return { items, nextCursor: null };
  },

  [RustCommand.AssistantConversation]: ({
    conversationId,
  }: Args): AssistantConversation => {
    requireAssistantLogin();
    return requireConversation(conversationId as string);
  },

  [RustCommand.AssistantConversationCreate]: ({
    serverId,
    modelId,
    title,
  }: Args): AssistantConversationSummary => {
    const session = requireAssistantLogin();
    if (!session.acceptedVersion) {
      throw 'Assistant consent_required: accept the current consent term first';
    }
    const model =
      ASSISTANT_MODELS.find(candidate => candidate.id === modelId) ??
      ASSISTANT_MODELS[0];
    const summary: AssistantConversationSummary = {
      id: nextId(),
      paradigm: paradigmFor(serverId as number),
      model,
      selectionMode: modelId ? 'free' : 'assigned',
      title: (title as string | null) ?? null,
      createdAt: now(),
      updatedAt: now(),
    };
    conversations.set(summary.id, { ...summary, turns: [] });
    return summary;
  },

  [RustCommand.AssistantTurnAsk]: async ({
    serverId,
    database,
    conversationId,
    question,
    clientTurnId,
    onEvent,
  }: Args): Promise<AssistantTurnFinish> => {
    requireAssistantLogin();
    const conversation = requireConversation(conversationId as string);
    const text = String(question ?? '').trim();
    if (!text) throw 'Invalid query: Empty question';
    const entry = requireServerEntry(serverId as number);
    const table =
      requireDatabase(entry, database as string).tables[0]?.name ?? 'items';

    const channel = onEvent as Channel<AssistantTurnEvent> | undefined;
    const emit = (event: AssistantTurnEvent) => channel?.onmessage(event);
    const running = { cancelled: false };
    runningTurns.set(clientTurnId as string, running);

    const turnId = nextId();
    const attempts: AssistantAttempt[] = [];
    let status: AssistantTurnStatus = 'answered';
    try {
      emit({ type: 'turnOpened', turnId });
      emit({ type: 'context', tables: 6, truncated: false });

      const final = outputFor(text, table);
      // "erro" na pergunta: a primeira tentativa erra a coluna e é corrigida
      const plan: ModelOutput[] = /erro|error/i.test(text)
        ? [{ ...final, query: `SELECT nome FROM "${table}"` }, final]
        : [final];

      for (let index = 0; index < plan.length; index += 1) {
        const output = plan[index];
        const current = attempt(index, output);
        attempts.push(current);
        emit({ type: 'attemptStarted', attemptId: current.id, index });
        for (const piece of (output.query ?? output.explanation).match(
          /.{1,12}/g,
        ) ?? []) {
          if (running.cancelled) break;
          await sleep(STEP_MS / 3);
          emit({ type: 'delta', text: piece });
        }
        if (running.cancelled) {
          status = 'cancelled';
          break;
        }
        emit({ type: 'output', attemptId: current.id, output });
        status = finishStatus(output);
        if (output.kind !== 'query') break;

        await sleep(STEP_MS);
        if (index < plan.length - 1) {
          emit({
            type: 'stageFailed',
            attemptId: current.id,
            stage: 'preflight',
            failureClass: 'unknown_object',
            message: 'Query error: column "nome" does not exist',
          });
          continue;
        }
        emit({
          type: 'plan',
          attemptId: current.id,
          plan: { estimatedRows: 1, estimatedCost: 35.5, plan: '[]' },
        });
        if (autoExecute()) {
          await sleep(STEP_MS);
          if (running.cancelled) {
            status = 'cancelled';
            break;
          }
          emit({ type: 'result', attemptId: current.id, result: resultFor() });
        }
      }
    } finally {
      runningTurns.delete(clientTurnId as string);
    }

    conversation.turns.push({
      id: turnId,
      index: conversation.turns.length,
      question: text,
      language: 'pt',
      status,
      createdAt: now(),
      attempts,
      feedback: null,
    });
    conversation.updatedAt = now();
    if (!conversation.title) conversation.title = text.slice(0, 60);

    const last = attempts.at(-1);
    return {
      turnId,
      status,
      attemptId: last?.id ?? null,
      output: last?.output ?? null,
      errorCode: null,
    };
  },

  [RustCommand.AssistantTurnCancel]: ({ clientTurnId }: Args): boolean => {
    const running = runningTurns.get(clientTurnId as string);
    if (!running) return false;
    running.cancelled = true;
    return true;
  },

  // Só leitura, como no Rust: o resto é recusado pelo guard
  [RustCommand.AssistantExecute]: ({ query }: Args): QueryResult => {
    if (!/^\s*(select|with|values)\b/i.test(String(query ?? ''))) {
      throw 'Guard rejected: only read queries (SELECT) are allowed';
    }
    return resultFor();
  },
};
