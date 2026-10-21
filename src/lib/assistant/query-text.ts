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

/** A consulta para leitura: no Mongo, o JSON indentado; o resto, como veio */
export function formatQueryText(
  language: QueryLanguage | null,
  text: string,
): string {
  if (language !== 'mongo' || !parseOperation(text)) return text;
  return JSON.stringify(JSON.parse(text), null, 2);
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
  if (language !== 'mongo') return text;
  return mongoShellCommand(text) ?? text;
}

/** Como a tela chama as relações do banco */
export function relationNouns(language: QueryLanguage | null): {
  one: string;
  many: string;
} {
  return language === 'mongo'
    ? { one: 'coleção', many: 'coleções' }
    : { one: 'tabela', many: 'tabelas' };
}
