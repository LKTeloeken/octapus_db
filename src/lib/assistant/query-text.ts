import type { QueryLanguage } from '@/api/types/capabilities.types';

/**
 * O texto da consulta do assistente, conforme a linguagem do banco
 * (`queryLanguage` das capabilities).
 *
 * No Mongo a consulta é a operação do contrato (`mongo-operation.schema.json`):
 * um JSON com `collection`, `operation` e os campos dela. Na tela ele aparece
 * indentado; no editor (C-10) vira o comando do shell que o editor do app roda
 * (`db.<coleção>.find(…)`). O código (`kind: code`) do Mongo já vem como
 * comando do shell e passa como está.
 *
 * No Redis a consulta é o plano de passos do contrato
 * (`redis-plan.schema.json`), também em JSON. O editor do Redis roda um
 * comando por vez, então só o plano de um passo sem `foreach` abre nele (como
 * a linha do `redis-cli`); o código do Redis já vem como comandos e abre se
 * for um só.
 */

type MongoOperation = {
  collection: string;
  operation: string;
  filter?: unknown;
  projection?: unknown;
  sort?: unknown;
  limit?: unknown;
  skip?: unknown;
  pipeline?: unknown;
  field?: unknown;
};

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

function parseOperation(text: string): MongoOperation | null {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return null;
  }
  if (
    !isObject(value) ||
    typeof value.collection !== 'string' ||
    typeof value.operation !== 'string'
  ) {
    return null;
  }
  return value as MongoOperation;
}

const json = (value: unknown) => JSON.stringify(value ?? {}, null, 2);

type RedisStep = {
  id: string;
  command: string;
  args: string[];
  foreach: string | null;
};

type RedisPlan = { steps: RedisStep[]; output: string };

function parsePlan(text: string): RedisPlan | null {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return null;
  }
  if (!isObject(value) || !Array.isArray(value.steps)) return null;
  const valid = value.steps.every(
    step =>
      isObject(step) &&
      typeof step.command === 'string' &&
      Array.isArray(step.args) &&
      step.args.every(arg => typeof arg === 'string'),
  );
  return valid ? (value as RedisPlan) : null;
}

/** A consulta para leitura: no Mongo e no Redis, o JSON indentado; o resto, como veio */
export function formatQueryText(
  language: QueryLanguage | null,
  text: string,
): string {
  const parsed =
    language === 'mongo'
      ? parseOperation(text)
      : language === 'redis_plan'
        ? parsePlan(text)
        : null;
  if (!parsed) return text;
  return JSON.stringify(JSON.parse(text), null, 2);
}

/** Um argumento como no `redis-cli`: entre aspas quando precisa */
function cliArg(arg: string): string {
  if (arg !== '' && !/[\s"'\\]/.test(arg)) return arg;
  return `"${arg.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

/**
 * O plano como a linha de comando do editor do Redis — só o de um passo sem
 * `foreach` (o editor roda um comando por vez). `null` para o resto.
 */
export function redisCommandLine(text: string): string | null {
  const plan = parsePlan(text);
  if (!plan || plan.steps.length !== 1 || plan.steps[0].foreach != null) {
    return null;
  }
  const [step] = plan.steps;
  return [step.command, ...step.args.map(cliArg)].join(' ');
}

/**
 * A operação como comando do editor do Mongo. `find` com `sort`, `skip` ou
 * `limit` vira `aggregate` (o editor só aceita filtro e projeção no `find`),
 * na mesma ordem do servidor: filtro, ordem, salto, limite, projeção.
 */
export function mongoShellCommand(text: string): string | null {
  const op = parseOperation(text);
  if (!op) return null;
  const target = `db.${op.collection}`;

  switch (op.operation) {
    case 'find': {
      if (op.sort == null && op.skip == null && op.limit == null) {
        const projection =
          op.projection == null ? '' : `, ${json(op.projection)}`;
        return `${target}.find(${json(op.filter)}${projection})`;
      }
      const pipeline: unknown[] = [{ $match: op.filter ?? {} }];
      if (op.sort != null) pipeline.push({ $sort: op.sort });
      if (op.skip != null) pipeline.push({ $skip: op.skip });
      if (op.limit != null) pipeline.push({ $limit: op.limit });
      if (op.projection != null) pipeline.push({ $project: op.projection });
      return `${target}.aggregate(${json(pipeline)})`;
    }
    case 'aggregate':
      return `${target}.aggregate(${json(op.pipeline ?? [])})`;
    case 'countDocuments':
      return `${target}.countDocuments(${json(op.filter)})`;
    case 'distinct':
      return `${target}.distinct(${JSON.stringify(op.field ?? '')}, ${json(op.filter)})`;
    default:
      return null;
  }
}

/** O texto que vai para a aba do editor ao "Abrir no editor" */
export function editorText(
  language: QueryLanguage | null,
  text: string,
): string {
  if (language === 'mongo') return mongoShellCommand(text) ?? text;
  if (language === 'redis_plan') return redisCommandLine(text) ?? text.trim();
  return text;
}

/**
 * Dá para abrir no editor? No Redis, só o que vira um comando: o plano de um
 * passo ou o código de uma linha (o editor junta as linhas num comando só).
 */
export function opensInEditor(
  language: QueryLanguage | null,
  text: string,
): boolean {
  if (language !== 'redis_plan') return true;
  if (parsePlan(text)) return redisCommandLine(text) !== null;
  return !text.trim().includes('\n');
}

/** Como a tela chama as relações do banco */
export function relationNouns(language: QueryLanguage | null): {
  one: string;
  many: string;
} {
  if (language === 'mongo') return { one: 'coleção', many: 'coleções' };
  if (language === 'redis_plan') {
    return { one: 'padrão de chave', many: 'padrões de chave' };
  }
  return { one: 'tabela', many: 'tabelas' };
}
