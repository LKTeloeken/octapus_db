import type { Channel } from '@tauri-apps/api/core';
import { RustCommand } from '@/api/commands';
import type {
  AssistantAttempt,
  AssistantConversation,
  AssistantConversationPage,
  AssistantConversationSummary,
  AssistantFeedbackInput,
  AssistantMention,
  AssistantTurn,
  AssistantTurnEvent,
  AssistantTurnFinish,
  AssistantTurnStatus,
  AttemptOutcome,
  ModelOutput,
} from '@/api/types/assistant.types';
import type { QueryLanguage } from '@/api/types/capabilities.types';
import { FLAT_SCHEMA } from '@/api/types/catalog.types';
import type { QueryOptions, QueryResult } from '@/api/types/query.types';
import {
  commentTooLong,
  FEEDBACK_COMMENT_MAX,
  normalizeFeedback,
} from '@/lib/assistant/feedback';
import {
  CAPABILITIES,
  findServerEntry,
  requireDatabase,
  requireServerEntry,
} from './data';
import {
  ASSISTANT_MODELS,
  assistantQuotaExhausted,
  requireAssistantLogin,
  spendAssistantQuota,
} from './assistant';
import { mockCatalog } from './catalog';
import { SAAS_DATABASE } from './data/tenants';
import type { MockHandler } from './handlers';

/**
 * Conversas e turnos do assistente simulados (B6). O turno emite os mesmos
 * eventos do backend, com pausas, para a tela ver o andamento:
 *
 * - pergunta curta (< 12 caracteres) → esclarecimento;
 * - pede escrita ("apagar", "excluir", "delete", "insert", "cadastrar",
 *   "atualizar") → código, nunca executado;
 * - menciona "erro" → a primeira tentativa falha no preflight e a correção
 *   passa (o ciclo do C-11); "esgot" → todas falham e o limite esgota (RN-24);
 * - "lista"/"todos" → listagem de 450 linhas, paginada pelo `assistant_execute`;
 * - "espera"/"lento" → rate limit (`waiting` de 3 s) antes da tentativa;
 * - "falha" → o provedor falha (`failed`, `ASSISTANT_PROVIDER_ERROR`);
 * - "cota" → gasta a cota inteira: daí em diante, `quota_exceeded`;
 * - o resto → contagem da primeira tabela do database.
 *
 * No Mongo (`queryLanguage: 'mongo'` das capabilities) as respostas são a
 * operação do contrato em JSON (`find`, `aggregate`, `countDocuments`) e o
 * código vem como comando do shell; o "erro" lê uma coleção que não existe.
 * No Redis (`redis_plan`) são o plano de passos: `SCAN` do prefixo (a
 * contagem vem no total) ou `SCAN` + `foreach HMGET` (a listagem); o código
 * vem como comando do `redis-cli` e o "erro" é um `HGET` sem o campo, que o
 * preflight pega pela aridade.
 *
 * A avaliação (👍/👎) segue as regras da A8: termo aceito, turno com
 * tentativa concluída, comentário até 2000 caracteres. Para ver o "desfazer"
 * da tela, ligue a falha no painel do mock.
 *
 * Como na API real, o `delta` é o JSON da resposta em pedaços. No database
 * `saas` (5.000 schemas) o contexto sai recortado (`truncated`).
 *
 * As menções com `@` (`mentions`) são conferidas no catálogo do mock, como no
 * backend: a que não existe volta em `missingMentions`; o `@schema` com mais
 * de [`LISTED_RELATIONS`] relações volta em `shortenedMentions` (a lista dos
 * nomes não coube inteira).
 *
 * A execução automática segue a preferência salva (`assistantAutoExecute`):
 * ligada, vem o resultado; desligada, só o plano. As conversas ficam em
 * memória (somem no reload), como o histórico que no app real mora na API.
 */

type Args = Record<string, unknown>;

/** As menções da pergunta conferidas no catálogo do mock (sem catálogo: nada) */
function checkMentions(
  serverId: number,
  database: string,
  mentions: AssistantMention[],
  schemas: string[],
): { missing: AssistantMention[]; shortened: AssistantMention[] } {
  const caps = CAPABILITIES[requireServerEntry(serverId).server.dbType];
  if (!caps.hasCatalog || mentions.length === 0) {
    return { missing: [], shortened: [] };
  }
  const searchPath = caps.hasSchemas
    ? schemas.length > 0
      ? schemas
      : ['public']
    : [FLAT_SCHEMA];
  const checks = mockCatalog.confirmMentions(
    serverId,
    database,
    mentions,
    searchPath,
  );
  return {
    missing: checks.filter(check => !check.found).map(check => check.mention),
    shortened: checks.flatMap(({ found }) =>
      found && found.table === null && found.size > LISTED_RELATIONS
        ? [{ schema: found.schema, table: null }]
        : [],
    ),
  };
}

const STEP_MS = 180;
/** Relações de um `@schema` que cabem na lista do contexto simulado */
const LISTED_RELATIONS = 40;
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

type Paradigm = 'postgres' | 'sqlite' | 'mongodb' | 'redis';

/** Como o `assistant_db_type` do Rust: decide pelas capabilities */
function paradigmFor(serverId: number): Paradigm {
  const { dbType } = requireServerEntry(serverId).server;
  if (!CAPABILITIES[dbType].supportsAssistant) {
    throw `Unsupported type: the assistant does not support ${dbType} yet`;
  }
  return dbType as Paradigm;
}

/** Sem servidor (testes antigos do mock), SQL */
function languageFor(serverId: unknown): QueryLanguage {
  const entry =
    typeof serverId === 'number' ? findServerEntry(serverId) : undefined;
  return entry ? CAPABILITIES[entry.server.dbType].queryLanguage : 'sql';
}

const mongoOp = (fields: Record<string, unknown>) => JSON.stringify(fields);

type PlanStep = {
  id: string;
  command: string;
  args: string[];
  foreach: string | null;
};

/** O plano do Redis do contrato; o último passo é o `output` */
const redisPlan = (...steps: PlanStep[]) =>
  JSON.stringify({ steps, output: steps[steps.length - 1].id });

/** O prefixo de um grupo de chaves do mock (`user:` → `user`) */
const keyPrefix = (table: string) => table.replace(/:$/, '');

function redisOutputFor(question: string, table: string): ModelOutput {
  const text = question.toLowerCase();
  const prefix = keyPrefix(table);
  const scan: PlanStep = {
    id: 'keys',
    command: 'SCAN',
    args: ['0', 'MATCH', `${prefix}:*`, 'TYPE', 'hash'],
    foreach: null,
  };
  if (question.trim().length < 12) {
    return {
      kind: 'clarification',
      language: null,
      query: null,
      explanation: '',
      assumptions: [],
      clarificationQuestion: `Sobre qual padrão de chave é a pergunta? Por exemplo, "${prefix}:{id}".`,
    };
  }
  if (/apag|exclu|delet|insert|cadastr|atualiz/.test(text)) {
    return {
      kind: 'code',
      language: 'redis_plan',
      query: `DEL ${prefix}:1`,
      explanation: `Remove a chave ${prefix}:1. O assistente não executa código.`,
      assumptions: [`"${prefix}:1" é a chave do registro`],
      clarificationQuestion: null,
    };
  }
  if (/lista|todos|todas/.test(text)) {
    return {
      kind: 'query',
      language: 'redis_plan',
      query: redisPlan(scan, {
        id: 'fields',
        command: 'HMGET',
        args: ['$item', 'nome', 'email'],
        foreach: 'keys',
      }),
      explanation: `Lista as chaves ${prefix}:{id} com o nome e o e-mail de cada uma.`,
      assumptions: ['Os hashes guardam os campos "nome" e "email"'],
      clarificationQuestion: null,
    };
  }
  return {
    kind: 'query',
    language: 'redis_plan',
    query: redisPlan(scan),
    explanation: `Percorre as chaves ${prefix}:{id}; o total é a contagem.`,
    assumptions: ['Todo hash com esse prefixo conta'],
    clarificationQuestion: null,
  };
}

function requireConversation(id: string): AssistantConversation {
  const conversation = conversations.get(id);
  if (!conversation) throw 'Assistant not_found: conversation not found';
  return conversation;
}

function requireTurn(turnId: string): AssistantTurn {
  for (const conversation of Array.from(conversations.values())) {
    const turn = conversation.turns.find(item => item.id === turnId);
    if (turn) return turn;
  }
  throw 'Assistant not_found: turn not found';
}

function outputFor(
  question: string,
  table: string,
  language: QueryLanguage,
): ModelOutput {
  if (language === 'redis_plan') return redisOutputFor(question, table);
  const text = question.toLowerCase();
  const mongo = language === 'mongo';
  if (question.trim().length < 12) {
    return {
      kind: 'clarification',
      language: null,
      query: null,
      explanation: '',
      assumptions: [],
      clarificationQuestion: `Sobre qual ${mongo ? 'coleção' : 'tabela'} é a pergunta? Por exemplo, "${table}".`,
    };
  }
  if (/apag|exclu|delet|insert|cadastr|atualiz/.test(text)) {
    return {
      kind: 'code',
      language,
      query: mongo
        ? `db.${table}.deleteOne({ _id: 1 })`
        : `DELETE FROM "${table}" WHERE id = 1`,
      explanation: 'Remove a linha de id 1. O assistente não executa código.',
      assumptions: ['"id" identifica a linha'],
      clarificationQuestion: null,
    };
  }
  if (/lista|todos|todas/.test(text)) {
    return {
      kind: 'query',
      language,
      query: mongo
        ? mongoOp({
            collection: table,
            operation: 'find',
            projection: { nome: 1, email: 1 },
            sort: { _id: 1 },
          })
        : `SELECT id, nome, email\nFROM "${table}"\nORDER BY id`,
      explanation: `Lista as linhas de ${table}, da mais antiga para a mais nova.`,
      assumptions: ['"nome" e "email" são as colunas que interessam'],
      clarificationQuestion: null,
    };
  }
  return {
    kind: 'query',
    language,
    query: mongo
      ? mongoOp({ collection: table, operation: 'countDocuments', filter: {} })
      : `SELECT count(*) AS total FROM "${table}"`,
    explanation: `Conta as linhas de ${table}.`,
    assumptions: ['Todas as linhas contam, sem filtro de data'],
    clarificationQuestion: null,
  };
}

/** Linhas de uma listagem ("lista", "todos"): o bastante para paginar */
const LIST_ROWS = 450;
const PAGE_LIMIT = 200;

/** Contagem (1 linha) ou uma listagem paginada por `offset`/`limit` */
function resultFor(
  query: string,
  offset: number,
  limit: number,
  language: QueryLanguage = 'sql',
): QueryResult {
  if (language === 'mongo') return mongoResultFor(query, offset, limit);
  if (language === 'redis_plan') return redisResultFor(query, offset, limit);
  if (/count\(/i.test(query)) {
    return {
      columns: [{ name: 'total', typeName: 'int8', typeOid: 20 }],
      rows: offset === 0 ? [['1284']] : [],
      rowCount: offset === 0 ? 1 : 0,
      totalCount: null,
      hasMore: false,
      executionTimeMs: 12,
      editableInfo: null,
    };
  }
  const end = Math.min(LIST_ROWS, offset + limit);
  const rows: (string | null)[][] = [];
  for (let id = offset + 1; id <= end; id += 1) {
    rows.push([
      String(id),
      `Cliente ${id}`,
      id % 7 === 0 ? null : `c${id}@exemplo.com`,
    ]);
  }
  return {
    columns: [
      { name: 'id', typeName: 'int4', typeOid: 23 },
      { name: 'nome', typeName: 'text', typeOid: 25 },
      { name: 'email', typeName: 'text', typeOid: 25 },
    ],
    rows,
    rowCount: rows.length,
    totalCount: null,
    hasMore: end < LIST_ROWS,
    executionTimeMs: 18,
    editableInfo: null,
  };
}

/** O mesmo, com a cara do Mongo: `count` ou documentos com `_id` */
function mongoResultFor(
  query: string,
  offset: number,
  limit: number,
): QueryResult {
  if (/"countDocuments"/.test(query)) {
    return {
      columns: [{ name: 'count', typeName: 'long', typeOid: null }],
      rows: offset === 0 ? [['1284']] : [],
      rowCount: offset === 0 ? 1 : 0,
      totalCount: null,
      hasMore: false,
      executionTimeMs: 9,
      editableInfo: null,
    };
  }
  const end = Math.min(LIST_ROWS, offset + limit);
  const rows: (string | null)[][] = [];
  for (let id = offset + 1; id <= end; id += 1) {
    rows.push([
      `66f1c0de${id.toString(16).padStart(16, '0')}`,
      `Cliente ${id}`,
      id % 7 === 0 ? null : `c${id}@exemplo.com`,
    ]);
  }
  return {
    columns: [
      { name: '_id', typeName: 'objectId', typeOid: null },
      { name: 'nome', typeName: 'string', typeOid: null },
      { name: 'email', typeName: 'string', typeOid: null },
    ],
    rows,
    rowCount: rows.length,
    totalCount: null,
    hasMore: end < LIST_ROWS,
    executionTimeMs: 14,
    editableInfo: null,
  };
}

/**
 * O mesmo, com a cara do Redis: o `SCAN` dá uma chave por linha e o total;
 * o `foreach HMGET` dá a chave e os campos pedidos
 */
function redisResultFor(
  query: string,
  offset: number,
  limit: number,
): QueryResult {
  const listing = /"HMGET"/.test(query);
  const total = listing ? LIST_ROWS : 1284;
  const end = Math.min(total, offset + limit);
  const rows: (string | null)[][] = [];
  for (let id = offset + 1; id <= end; id += 1) {
    const key = `user:${id}`;
    rows.push(
      listing
        ? [key, `Cliente ${id}`, id % 7 === 0 ? null : `c${id}@exemplo.com`]
        : [key],
    );
  }
  const names = listing ? ['key', 'nome', 'email'] : ['key'];
  return {
    columns: names.map(name => ({ name, typeName: 'string', typeOid: null })),
    rows,
    rowCount: rows.length,
    totalCount: total,
    hasMore: end < total,
    executionTimeMs: 7,
    editableInfo: null,
  };
}

/** Os comandos de leitura que o plano do Redis aceita (`redisdb/plan.rs`), resumidos */
const REDIS_READS = new Set([
  'SCAN',
  'EXISTS',
  'TYPE',
  'TTL',
  'DBSIZE',
  'GET',
  'MGET',
  'STRLEN',
  'HGET',
  'HMGET',
  'HGETALL',
  'HKEYS',
  'HLEN',
  'LRANGE',
  'LLEN',
  'SMEMBERS',
  'SCARD',
  'ZRANGE',
  'ZSCORE',
  'ZCARD',
  'ZCOUNT',
  'XRANGE',
  'XLEN',
]);

/** O guard do Redis, simplificado: o plano do contrato e só leitura */
function redisGuard(text: string): string | null {
  let plan: { steps?: unknown };
  try {
    plan = JSON.parse(text);
  } catch {
    return 'the plan is not a valid redis-plan JSON (malformed JSON)';
  }
  if (!Array.isArray(plan.steps) || plan.steps.length === 0) {
    return 'the plan has no steps';
  }
  for (const step of plan.steps as Partial<PlanStep>[]) {
    const command = String(step.command ?? '');
    if (command === 'KEYS') {
      return 'KEYS is not allowed: use SCAN with MATCH, the app walks the cursor';
    }
    if (/^(EVAL|EVALSHA|FCALL|FUNCTION|SCRIPT)$/.test(command)) {
      return `${command} runs a script on the server and is not allowed`;
    }
    if (!REDIS_READS.has(command)) {
      return `${command} is not one of the read commands a plan can use`;
    }
    if (command === 'SCAN' && step.foreach != null) {
      return 'SCAN cannot run inside a foreach: scan once and iterate over its keys';
    }
  }
  return null;
}

/** Estágios e operadores que o guard do Mongo recusa (`guard/mongo.rs`) */
const MONGO_FORBIDDEN = [
  '$out',
  '$merge',
  '$where',
  '$function',
  '$accumulator',
];

/** O guard do Mongo, simplificado: a operação do contrato e só leitura */
function mongoGuard(text: string): string | null {
  let op: { operation?: unknown };
  try {
    op = JSON.parse(text);
  } catch {
    return 'the operation is not a valid mongo-operation JSON';
  }
  const operations = ['find', 'aggregate', 'countDocuments', 'distinct'];
  if (typeof op.operation !== 'string' || !operations.includes(op.operation)) {
    return `operation ${JSON.stringify(op.operation)} is not allowed: use find, aggregate, countDocuments or distinct`;
  }
  const found = MONGO_FORBIDDEN.find(key => text.includes(`"${key}":`));
  return found ? `${found} is not allowed` : null;
}

/** O outcome que o app mandaria à API por tentativa */
function outcome(
  guardReason: string | null,
  autoExecute: boolean,
  extra: {
    preflight?: string;
    preflightClass?: 'unknown_object' | 'syntax';
    rowCount?: number;
  } = {},
): AttemptOutcome {
  const skipped = {
    ran: false,
    ok: null,
    failureClass: null,
    errorSanitized: null,
    durationMs: null,
  };
  if (guardReason) {
    return {
      guard: { allowed: false, reason: guardReason },
      preflight: skipped,
      execution: skipped,
      rowCount: null,
      autoExecute,
      userEdited: false,
    };
  }
  const preflight = extra.preflight
    ? {
        ran: true,
        ok: false,
        failureClass: extra.preflightClass ?? 'unknown_object',
        errorSanitized: extra.preflight,
        durationMs: 4,
      }
    : {
        ran: true,
        ok: true,
        failureClass: null,
        errorSanitized: null,
        durationMs: 4,
      };
  const executed = extra.rowCount !== undefined;
  return {
    guard: { allowed: true, reason: null },
    preflight,
    execution: executed
      ? {
          ran: true,
          ok: true,
          failureClass: null,
          errorSanitized: null,
          durationMs: 12,
        }
      : skipped,
    rowCount: executed ? (extra.rowCount ?? null) : null,
    autoExecute,
    userEdited: false,
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
    // Cópia, como o IPC: a conversa em memória continua mudando depois
    return structuredClone(requireConversation(conversationId as string));
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
    schemas,
    mentions,
    onEvent,
  }: Args): Promise<AssistantTurnFinish> => {
    requireAssistantLogin();
    const conversation = requireConversation(conversationId as string);
    const text = String(question ?? '').trim();
    if (!text) throw 'Invalid query: Empty question';
    if (/cota/i.test(text)) spendAssistantQuota(Number.MAX_SAFE_INTEGER);
    if (assistantQuotaExhausted()) {
      throw 'Assistant quota_exceeded: participant token quota exhausted';
    }
    const entry = requireServerEntry(serverId as number);
    const table =
      requireDatabase(entry, database as string).tables[0]?.name ?? 'items';
    const language = languageFor(serverId);
    const { missing, shortened } = checkMentions(
      serverId as number,
      database as string,
      (mentions as AssistantMention[] | null) ?? [],
      (schemas as string[] | null) ?? [],
    );

    const channel = onEvent as Channel<AssistantTurnEvent> | undefined;
    const emit = (event: AssistantTurnEvent) => channel?.onmessage(event);
    const running = { cancelled: false };
    runningTurns.set(clientTurnId as string, running);
    const auto = autoExecute();

    const turnId = nextId();
    const attempts: AssistantAttempt[] = [];
    let status: AssistantTurnStatus = 'answered';
    let errorCode: string | null = null;
    try {
      emit({ type: 'turnOpened', turnId });
      await sleep(STEP_MS / 2);
      emit({
        type: 'context',
        tables: 6,
        // O saas (5.000 schemas) não cabe: recorte (RN-26)
        truncated: database === SAAS_DATABASE || shortened.length > 0,
        missingMentions: missing,
        shortenedMentions: shortened,
      });

      if (/espera|lento/i.test(text)) {
        emit({ type: 'waiting', seconds: 3 });
        await sleep(3_000);
      }

      const final = outputFor(text, table, language);
      const redis = language === 'redis_plan';
      const broken =
        language === 'mongo'
          ? {
              ...final,
              query: mongoOp({ collection: 'pedidos', operation: 'find' }),
            }
          : redis
            ? {
                ...final,
                query: redisPlan({
                  id: 'nome',
                  command: 'HGET',
                  args: [`${keyPrefix(table)}:1`],
                  foreach: null,
                }),
              }
            : { ...final, query: `SELECT nome FROM "${table}"` };
      // "erro": a primeira falha e a correção passa; "esgot": todas falham
      const plan: ModelOutput[] = /esgot/i.test(text)
        ? [broken, broken, broken]
        : /erro|error/i.test(text)
          ? [broken, final]
          : [final];

      // Cancelar acorda a espera, como no Rust
      if (running.cancelled) status = 'cancelled';

      for (
        let index = 0;
        index < plan.length && !running.cancelled;
        index += 1
      ) {
        const output = plan[index];
        const current = attempt(index, output);
        attempts.push(current);
        spendAssistantQuota(2_580);
        emit({ type: 'attemptStarted', attemptId: current.id, index });
        await sleep(STEP_MS);

        if (/falha/i.test(text)) {
          current.status = 'provider_error';
          current.output = null;
          status = 'failed';
          errorCode = 'ASSISTANT_PROVIDER_ERROR';
          break;
        }

        // O delta é o texto cru da saída estruturada: JSON parcial
        for (const piece of JSON.stringify(output).match(/[\s\S]{1,24}/g) ??
          []) {
          if (running.cancelled) break;
          await sleep(STEP_MS / 6);
          emit({ type: 'delta', text: piece });
        }
        if (running.cancelled) {
          // Sem o `output`, não há resposta: o turno acaba sem consulta
          current.status = 'cancelled';
          current.output = null;
          status = 'cancelled';
          break;
        }
        emit({ type: 'output', attemptId: current.id, output });
        status = finishStatus(output);
        if (output.kind !== 'query') {
          current.outcome = outcome(`kind_${output.kind}`, auto);
          break;
        }

        await sleep(STEP_MS);
        if (output === broken) {
          emit({
            type: 'stageFailed',
            attemptId: current.id,
            stage: 'preflight',
            failureClass: redis ? 'syntax' : 'unknown_object',
            message:
              language === 'mongo'
                ? 'Query error: collection "pedidos" does not exist'
                : redis
                  ? "Query error: ERR wrong number of arguments for 'hget' command"
                  : 'Query error: column "nome" does not exist',
          });
          current.outcome = outcome(null, auto, {
            preflight:
              language === 'mongo'
                ? 'NamespaceNotFound (26): collection "pedidos" does not exist'
                : redis
                  ? "ERR: wrong number of arguments for 'hget' command"
                  : 'column ? does not exist',
            preflightClass: redis ? 'syntax' : 'unknown_object',
          });
          if (index === plan.length - 1) status = 'exhausted';
          continue;
        }
        emit({
          type: 'plan',
          attemptId: current.id,
          // Mongo e Redis não estimam: o caminho do plano, ou os comandos
          plan:
            language === 'mongo'
              ? { estimatedRows: null, estimatedCost: null, plan: 'COLLSCAN' }
              : redis
                ? {
                    estimatedRows: null,
                    estimatedCost: null,
                    plan: /"HMGET"/.test(output.query ?? '')
                      ? 'SCAN → foreach HMGET'
                      : 'SCAN',
                  }
                : { estimatedRows: 1, estimatedCost: 35.5, plan: '[]' },
        });
        if (auto) {
          await sleep(STEP_MS);
          if (running.cancelled) {
            status = 'cancelled';
            break;
          }
          const result = resultFor(output.query ?? '', 0, PAGE_LIMIT, language);
          emit({ type: 'result', attemptId: current.id, result });
          current.outcome = outcome(null, true, { rowCount: result.rowCount });
        } else {
          current.outcome = outcome(null, false);
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

    const last = attempts.filter(item => item.output).at(-1);
    return {
      turnId,
      status,
      attemptId: last?.id ?? null,
      output: last?.output ?? null,
      errorCode,
    };
  },

  [RustCommand.AssistantTurnCancel]: ({ clientTurnId }: Args): boolean => {
    const running = runningTurns.get(clientTurnId as string);
    if (!running) return false;
    running.cancelled = true;
    return true;
  },

  [RustCommand.AssistantFeedbackSet]: ({ turnId, feedback }: Args): void => {
    const session = requireAssistantLogin();
    const input = feedback as AssistantFeedbackInput;
    if (input.comment && commentTooLong(input.comment)) {
      throw `Assistant invalid_request: comment longer than ${FEEDBACK_COMMENT_MAX} characters`;
    }
    if (!session.acceptedVersion) {
      throw 'Assistant consent_required: accept the current consent term first';
    }
    const turn = requireTurn(turnId as string);
    if (!turn.attempts.some(item => item.status !== 'generating')) {
      throw 'Assistant conflict: the turn has no finished attempt';
    }
    const normalized = normalizeFeedback(input);
    if (
      normalized.attemptId &&
      !turn.attempts.some(item => item.id === normalized.attemptId)
    ) {
      throw 'Assistant invalid_request: attemptId is not an attempt of this turn';
    }
    turn.feedback = {
      rating: normalized.rating,
      reasons: normalized.reasons,
      comment: normalized.comment,
      attemptId: normalized.attemptId,
      createdAt: turn.feedback?.createdAt ?? now(),
      updatedAt: now(),
    };
  },

  // Sem avaliação não é erro; não exige o termo (como na API)
  [RustCommand.AssistantFeedbackClear]: ({ turnId }: Args): void => {
    requireAssistantLogin();
    requireTurn(turnId as string).feedback = null;
  },

  // Só leitura, como no Rust: o resto é recusado pelo guard
  [RustCommand.AssistantExecute]: ({
    serverId,
    query,
    page,
  }: Args): QueryResult => {
    const text = String(query ?? '');
    const language = languageFor(serverId);
    if (language === 'mongo') {
      const rejected = mongoGuard(text);
      if (rejected) throw `Guard rejected: ${rejected}`;
    } else if (language === 'redis_plan') {
      const rejected = redisGuard(text);
      if (rejected) throw `Guard rejected: ${rejected}`;
    } else if (!/^\s*(select|with|values)\b/i.test(text)) {
      throw 'Guard rejected: only read queries (SELECT) are allowed';
    }
    const { offset = 0, limit = PAGE_LIMIT } = (page ?? {}) as QueryOptions;
    return resultFor(text, offset, limit, language);
  },
};
