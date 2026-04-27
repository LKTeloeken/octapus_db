import { useMemo } from 'react';
import CodeMirror from '@uiw/react-codemirror';
import { oneDark } from '@codemirror/theme-one-dark';
import {
  autocompletion,
  completionStatus,
  Completion,
  CompletionContext,
  CompletionResult,
  snippetCompletion,
  startCompletion,
} from '@codemirror/autocomplete';
import { keymap, placeholder, EditorView } from '@codemirror/view';
import { EditorState, Prec } from '@codemirror/state';
import { sql, PostgreSQL, SQLite, StandardSQL } from '@codemirror/lang-sql';
import { javascript } from '@codemirror/lang-javascript';

export type QueryDialect = 'postgres' | 'sqlite' | 'mongo';

export type QueryEditorTheme = 'light' | 'dark';

export interface QueryEditorColumn {
  name: string;
  type?: string;
  nullable?: boolean;
  description?: string;
}

export interface QueryEditorTable {
  name: string;
  schema?: string;
  columns?: QueryEditorColumn[];
  description?: string;
}

export interface QueryEditorCollection {
  name: string;
  fields?: QueryEditorColumn[];
  description?: string;
}

export interface QueryEditorSchema {
  tables?: QueryEditorTable[];
  collections?: QueryEditorCollection[];
  functions?: string[];
}

export interface QueryEditorProps {
  value: string;
  onChange: (value: string) => void;

  dialect: QueryDialect;
  schema?: QueryEditorSchema;

  height?: string;
  minHeight?: string;
  maxHeight?: string;

  theme?: QueryEditorTheme;
  readOnly?: boolean;
  autoFocus?: boolean;
  placeholderText?: string;

  fontSize?: number;

  runMode?: QueryEditorRunMode;
  onRun?: (query: string, context: QueryEditorRunContext) => void;

  className?: string;
}

export type QueryEditorRunMode = 'all' | 'selection' | 'selection-or-all';

export interface QueryEditorRunSelection {
  from: number;
  to: number;
  text: string;
}

export interface QueryEditorRunContext {
  source: 'all' | 'selection';
  selections: QueryEditorRunSelection[];
}

const POSTGRES_KEYWORDS = [
  'SELECT',
  'FROM',
  'WHERE',
  'INSERT',
  'INTO',
  'VALUES',
  'UPDATE',
  'SET',
  'DELETE',
  'CREATE',
  'ALTER',
  'DROP',
  'TABLE',
  'VIEW',
  'INDEX',
  'SEQUENCE',
  'SCHEMA',
  'DATABASE',
  'JOIN',
  'INNER JOIN',
  'LEFT JOIN',
  'RIGHT JOIN',
  'FULL JOIN',
  'CROSS JOIN',
  'ON',
  'GROUP BY',
  'ORDER BY',
  'HAVING',
  'LIMIT',
  'OFFSET',
  'RETURNING',
  'DISTINCT',
  'UNION',
  'UNION ALL',
  'EXCEPT',
  'INTERSECT',
  'WITH',
  'AS',
  'CASE',
  'WHEN',
  'THEN',
  'ELSE',
  'END',
  'AND',
  'OR',
  'NOT',
  'NULL',
  'IS NULL',
  'IS NOT NULL',
  'TRUE',
  'FALSE',
  'LIKE',
  'ILIKE',
  'IN',
  'BETWEEN',
  'EXISTS',
  'ANY',
  'ALL',
  'CAST',
  'PRIMARY KEY',
  'FOREIGN KEY',
  'REFERENCES',
  'CONSTRAINT',
  'UNIQUE',
  'CHECK',
  'DEFAULT',
  'SERIAL',
  'BIGSERIAL',
  'UUID',
  'JSON',
  'JSONB',
  'TEXT',
  'VARCHAR',
  'INTEGER',
  'BIGINT',
  'BOOLEAN',
  'TIMESTAMP',
  'TIMESTAMPTZ',
  'DATE',
  'TIME',
  'NUMERIC',
];

const SQLITE_KEYWORDS = [
  'SELECT',
  'FROM',
  'WHERE',
  'INSERT',
  'INTO',
  'VALUES',
  'UPDATE',
  'SET',
  'DELETE',
  'CREATE',
  'ALTER',
  'DROP',
  'TABLE',
  'VIEW',
  'INDEX',
  'TRIGGER',
  'JOIN',
  'INNER JOIN',
  'LEFT JOIN',
  'CROSS JOIN',
  'ON',
  'GROUP BY',
  'ORDER BY',
  'HAVING',
  'LIMIT',
  'OFFSET',
  'DISTINCT',
  'UNION',
  'UNION ALL',
  'EXCEPT',
  'INTERSECT',
  'WITH',
  'AS',
  'CASE',
  'WHEN',
  'THEN',
  'ELSE',
  'END',
  'AND',
  'OR',
  'NOT',
  'NULL',
  'IS NULL',
  'IS NOT NULL',
  'TRUE',
  'FALSE',
  'LIKE',
  'GLOB',
  'IN',
  'BETWEEN',
  'EXISTS',
  'CAST',
  'PRIMARY KEY',
  'FOREIGN KEY',
  'REFERENCES',
  'CONSTRAINT',
  'UNIQUE',
  'CHECK',
  'DEFAULT',
  'AUTOINCREMENT',
  'INTEGER',
  'REAL',
  'TEXT',
  'BLOB',
  'NUMERIC',
];

const POSTGRES_FUNCTIONS = [
  'COUNT',
  'SUM',
  'AVG',
  'MIN',
  'MAX',
  'NOW',
  'CURRENT_DATE',
  'CURRENT_TIME',
  'CURRENT_TIMESTAMP',
  'COALESCE',
  'NULLIF',
  'LOWER',
  'UPPER',
  'LENGTH',
  'SUBSTRING',
  'TRIM',
  'ROUND',
  'RANDOM',
  'ARRAY_AGG',
  'JSON_AGG',
  'JSONB_AGG',
  'TO_CHAR',
  'TO_DATE',
  'DATE_TRUNC',
  'EXTRACT',
];

const SQLITE_FUNCTIONS = [
  'COUNT',
  'SUM',
  'AVG',
  'MIN',
  'MAX',
  'DATE',
  'TIME',
  'DATETIME',
  'JULIANDAY',
  'STRFTIME',
  'COALESCE',
  'NULLIF',
  'LOWER',
  'UPPER',
  'LENGTH',
  'SUBSTR',
  'TRIM',
  'ROUND',
  'RANDOM',
  'ABS',
  'IFNULL',
  'GROUP_CONCAT',
  'JSON_EXTRACT',
  'JSON_OBJECT',
  'JSON_ARRAY',
];

const MONGO_METHODS = [
  'find',
  'findOne',
  'insertOne',
  'insertMany',
  'updateOne',
  'updateMany',
  'deleteOne',
  'deleteMany',
  'aggregate',
  'countDocuments',
  'estimatedDocumentCount',
  'distinct',
  'createIndex',
  'dropIndex',
  'drop',
  'sort',
  'limit',
  'skip',
  'project',
  'toArray',
];

const MONGO_OPERATORS = [
  '$eq',
  '$ne',
  '$gt',
  '$gte',
  '$lt',
  '$lte',
  '$in',
  '$nin',
  '$and',
  '$or',
  '$not',
  '$nor',
  '$exists',
  '$type',
  '$regex',
  '$expr',
  '$all',
  '$elemMatch',
  '$size',
  '$set',
  '$unset',
  '$inc',
  '$mul',
  '$rename',
  '$min',
  '$max',
  '$push',
  '$pop',
  '$pull',
  '$addToSet',
  '$each',
  '$position',
  '$slice',
  '$sort',
  '$match',
  '$group',
  '$project',
  '$lookup',
  '$unwind',
  '$sort',
  '$limit',
  '$skip',
  '$count',
  '$facet',
  '$replaceRoot',
  '$replaceWith',
  '$addFields',
  '$set',
  '$out',
  '$merge',
];

function uniqueByLabel(items: Completion[]): Completion[] {
  const seen = new Set<string>();
  const result: Completion[] = [];

  for (const item of items) {
    const key = `${item.label}:${item.type ?? ''}`;

    if (!seen.has(key)) {
      seen.add(key);
      result.push(item);
    }
  }

  return result;
}

function createKeywordCompletions(keywords: string[]): Completion[] {
  return keywords.map(keyword => ({
    label: keyword,
    type: 'keyword',
    boost: 10,
  }));
}

function createFunctionCompletions(functions: string[]): Completion[] {
  return functions.map(fn => ({
    label: fn,
    type: 'function',
    apply: `${fn}()`,
    detail: 'function',
    boost: 8,
  }));
}

function createSqlSchemaCompletions(schema?: QueryEditorSchema): Completion[] {
  if (!schema?.tables?.length) {
    return [];
  }

  const completions: Completion[] = [];

  for (const table of schema.tables) {
    const fullTableName = table.schema
      ? `${table.schema}.${table.name}`
      : table.name;

    completions.push({
      label: fullTableName,
      type: 'class',
      detail: table.description ?? 'table',
      boost: 20,
    });

    completions.push({
      label: table.name,
      type: 'class',
      detail: table.description ?? 'table',
      boost: 18,
    });

    for (const column of table.columns ?? []) {
      completions.push({
        label: column.name,
        type: 'property',
        detail: column.type ?? 'column',
        info: column.description,
        boost: 12,
      });

      completions.push({
        label: `${table.name}.${column.name}`,
        type: 'property',
        detail: column.type ?? 'column',
        info: column.description,
        boost: 16,
      });

      if (table.schema) {
        completions.push({
          label: `${table.schema}.${table.name}.${column.name}`,
          type: 'property',
          detail: column.type ?? 'column',
          info: column.description,
          boost: 16,
        });
      }
    }
  }

  return uniqueByLabel(completions);
}

function createMongoSchemaCompletions(
  schema?: QueryEditorSchema,
): Completion[] {
  if (!schema?.collections?.length) {
    return [];
  }

  const completions: Completion[] = [];

  for (const collection of schema.collections) {
    completions.push({
      label: collection.name,
      type: 'class',
      detail: collection.description ?? 'collection',
      boost: 20,
    });

    completions.push({
      label: `db.${collection.name}`,
      type: 'variable',
      detail: 'collection',
      boost: 22,
    });

    for (const field of collection.fields ?? []) {
      completions.push({
        label: field.name,
        type: 'property',
        detail: field.type ?? 'field',
        info: field.description,
        boost: 14,
      });
    }
  }

  return uniqueByLabel(completions);
}

function getSqlTableColumnCompletions(
  token: string,
  schema?: QueryEditorSchema,
): Completion[] {
  if (!schema?.tables?.length || !token.endsWith('.')) {
    return [];
  }

  const tableOrSchema = token.slice(0, -1).toLowerCase();
  const completions: Completion[] = [];

  for (const table of schema.tables) {
    const tableName = table.name.toLowerCase();
    const fullTableName = table.schema
      ? `${table.schema}.${table.name}`.toLowerCase()
      : tableName;

    if (tableOrSchema !== tableName && tableOrSchema !== fullTableName) {
      continue;
    }

    for (const column of table.columns ?? []) {
      completions.push({
        label: column.name,
        type: 'property',
        detail: column.type ?? 'column',
        info: column.description,
        boost: 30,
      });
    }
  }

  return completions;
}

function getMongoFieldCompletions(
  token: string,
  schema?: QueryEditorSchema,
): Completion[] {
  if (!schema?.collections?.length || !token.endsWith('.')) {
    return [];
  }

  const prefix = token.slice(0, -1).toLowerCase();
  const completions: Completion[] = [];

  for (const collection of schema.collections) {
    const collectionName = collection.name.toLowerCase();

    if (
      prefix !== collectionName &&
      prefix !== `db.${collectionName}` &&
      prefix !== 'db'
    ) {
      continue;
    }

    if (prefix === 'db') {
      completions.push({
        label: collection.name,
        type: 'class',
        detail: 'collection',
        boost: 35,
      });

      continue;
    }

    for (const method of MONGO_METHODS) {
      completions.push({
        label: method,
        type: 'function',
        apply: `${method}()`,
        detail: 'MongoDB method',
        boost: 30,
      });
    }

    for (const field of collection.fields ?? []) {
      completions.push({
        label: field.name,
        type: 'property',
        detail: field.type ?? 'field',
        info: field.description,
        boost: 25,
      });
    }
  }

  return completions;
}

function getWordBeforeCursor(context: CompletionContext) {
  return context.matchBefore(/[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*\.?/);
}

function createSqlCompletionSource(options: {
  dialect: 'postgres' | 'sqlite';
  schema?: QueryEditorSchema;
}): (context: CompletionContext) => CompletionResult | null {
  const keywords =
    options.dialect === 'postgres' ? POSTGRES_KEYWORDS : SQLITE_KEYWORDS;

  const builtinFunctions =
    options.dialect === 'postgres' ? POSTGRES_FUNCTIONS : SQLITE_FUNCTIONS;

  const allFunctions = uniqueByLabel([
    ...createFunctionCompletions(builtinFunctions),
    ...createFunctionCompletions(options.schema?.functions ?? []),
  ]);

  const baseCompletions = uniqueByLabel([
    ...createKeywordCompletions(keywords),
    ...allFunctions,
    ...createSqlSchemaCompletions(options.schema),
    snippetCompletion('SELECT ${columns} FROM ${table};', {
      label: 'select',
      detail: 'SELECT query',
      type: 'keyword',
    }),
    snippetCompletion('INSERT INTO ${table} (${columns}) VALUES (${values});', {
      label: 'insert',
      detail: 'INSERT query',
      type: 'keyword',
    }),
    snippetCompletion('UPDATE ${table} SET ${column} = ${value} WHERE ${id};', {
      label: 'update',
      detail: 'UPDATE query',
      type: 'keyword',
    }),
    snippetCompletion('DELETE FROM ${table} WHERE ${condition};', {
      label: 'delete',
      detail: 'DELETE query',
      type: 'keyword',
    }),
  ]);

  return (context: CompletionContext): CompletionResult | null => {
    const updateSetContext = getUpdateSetColumnContext(
      context.state,
      context.pos,
    );

    if (updateSetContext) {
      const updateColumnCompletions = createUpdateSetColumnCompletions(
        options.schema,
        updateSetContext.table,
      );

      if (updateColumnCompletions.length > 0) {
        return {
          from: updateSetContext.from,
          options: updateColumnCompletions,
          validFor: /^[\w$"]*$/,
        };
      }
    }

    const word = getWordBeforeCursor(context);

    if (!word && !context.explicit) {
      return null;
    }

    const token = word?.text ?? '';
    const dotCompletions = getSqlTableColumnCompletions(token, options.schema);

    if (dotCompletions.length > 0 && word) {
      return {
        from: word.to,
        options: dotCompletions,
        validFor: /^[\w$]*$/,
      };
    }

    return {
      from: word?.from ?? context.pos,
      options: baseCompletions,
      validFor: /^[\w$.\s]*$/,
    };
  };
}

function createUpdateSetColumnCompletions(
  schema?: QueryEditorSchema,
  tableRef?: SqlTableRef,
): Completion[] {
  if (!schema?.tables?.length) {
    return [];
  }

  const matchingTables = schema.tables.filter(table => {
    if (!tableRef) {
      return true;
    }

    const tableName = normalizeSqlIdentifier(table.name);
    const schemaName = table.schema
      ? normalizeSqlIdentifier(table.schema)
      : undefined;

    const fullName = schemaName ? `${schemaName}.${tableName}` : tableName;

    return (
      tableName === tableRef.name ||
      fullName === tableRef.fullName ||
      (schemaName === tableRef.schema && tableName === tableRef.name)
    );
  });

  const tablesToUse =
    matchingTables.length > 0 ? matchingTables : schema.tables;

  const completions: Completion[] = [];

  for (const table of tablesToUse) {
    for (const column of table.columns ?? []) {
      completions.push({
        label: column.name,
        type: 'property',
        detail: column.type ?? 'column',
        info: column.description,
        boost: tableRef ? 50 : 30,
      });
    }
  }

  return uniqueByLabel(completions);
}

function createMongoCompletionSource(options: {
  schema?: QueryEditorSchema;
}): (context: CompletionContext) => CompletionResult | null {
  const baseCompletions = uniqueByLabel([
    {
      label: 'db',
      type: 'variable',
      detail: 'MongoDB database object',
      boost: 30,
    },
    ...createMongoSchemaCompletions(options.schema),
    ...MONGO_METHODS.map(method => ({
      label: method,
      type: 'function',
      apply: `${method}()`,
      detail: 'MongoDB method',
      boost: 15,
    })),
    ...MONGO_OPERATORS.map(operator => ({
      label: operator,
      type: 'constant',
      detail: 'MongoDB operator',
      boost: 20,
    })),
    snippetCompletion('db.${collection}.find({ ${filter} });', {
      label: 'find',
      detail: 'MongoDB find query',
      type: 'function',
    }),
    snippetCompletion('db.${collection}.findOne({ ${filter} });', {
      label: 'findOne',
      detail: 'MongoDB findOne query',
      type: 'function',
    }),
    snippetCompletion(
      'db.${collection}.aggregate([\n  { $match: { ${filter} } }\n]);',
      {
        label: 'aggregate',
        detail: 'MongoDB aggregation',
        type: 'function',
      },
    ),
    snippetCompletion(
      'db.${collection}.updateOne(\n  { ${filter} },\n  { $set: { ${field}: ${value} } }\n);',
      {
        label: 'updateOne',
        detail: 'MongoDB updateOne query',
        type: 'function',
      },
    ),
  ]);

  return (context: CompletionContext): CompletionResult | null => {
    const word = getWordBeforeCursor(context);

    if (!word && !context.explicit) {
      return null;
    }

    const token = word?.text ?? '';
    const dotCompletions = getMongoFieldCompletions(token, options.schema);

    if (dotCompletions.length > 0 && word) {
      return {
        from: word.to,
        options: dotCompletions,
        validFor: /^[\w$]*$/,
      };
    }

    return {
      from: word?.from ?? context.pos,
      options: baseCompletions,
      validFor: /^[\w$.\s]*$/,
    };
  };
}

function getSqlDialect(dialect: QueryDialect) {
  if (dialect === 'postgres') {
    return PostgreSQL;
  }

  if (dialect === 'sqlite') {
    return SQLite;
  }

  return StandardSQL;
}

function getEditorSelections(view: EditorView): QueryEditorRunSelection[] {
  return view.state.selection.ranges
    .filter(range => !range.empty)
    .map(range => ({
      from: range.from,
      to: range.to,
      text: view.state.sliceDoc(range.from, range.to),
    }))
    .filter(selection => selection.text.trim().length > 0);
}

function getRunnableQuery(
  view: EditorView,
  runMode: QueryEditorRunMode,
): {
  query: string;
  context: QueryEditorRunContext;
} | null {
  const selections = getEditorSelections(view);

  if (runMode === 'selection') {
    if (selections.length === 0) {
      return null;
    }

    return {
      query: selections.map(selection => selection.text).join('\n'),
      context: {
        source: 'selection',
        selections,
      },
    };
  }

  if (runMode === 'selection-or-all' && selections.length > 0) {
    return {
      query: selections.map(selection => selection.text).join('\n'),
      context: {
        source: 'selection',
        selections,
      },
    };
  }

  return {
    query: view.state.doc.toString(),
    context: {
      source: 'all',
      selections: [],
    },
  };
}

type SqlTokenKind =
  | 'identifier'
  | 'keyword'
  | 'number'
  | 'string'
  | 'punctuation'
  | 'operator';

interface SqlToken {
  text: string;
  upper: string;
  from: number;
  to: number;
  kind: SqlTokenKind;
}

interface SqlTableRef {
  name: string;
  schema?: string;
  fullName: string;
}

interface UpdateSetColumnContext {
  from: number;
  table?: SqlTableRef;
  reason: 'after-set' | 'after-comma' | 'partial-column';
}

const SQL_CLAUSE_TERMINATORS_AFTER_SET = new Set([
  'WHERE',
  'FROM',
  'RETURNING',
  'ORDER',
  'LIMIT',
  'GROUP',
  'HAVING',
  'UNION',
  'EXCEPT',
  'INTERSECT',
]);

const SQLITE_UPDATE_CONFLICT_TOKENS = new Set([
  'OR',
  'ROLLBACK',
  'ABORT',
  'REPLACE',
  'FAIL',
  'IGNORE',
]);

function isSqlIdentifierToken(token: SqlToken | undefined): boolean {
  if (!token) {
    return false;
  }

  return token.kind === 'identifier' || token.kind === 'keyword';
}

function normalizeSqlIdentifier(identifier: string): string {
  return identifier
    .replace(/^["'`\[]/, '')
    .replace(/["'`\]]$/, '')
    .toLowerCase();
}

function tokenizeSqlPrefix(input: string, offset = 0): SqlToken[] {
  const tokens: SqlToken[] = [];
  let index = 0;

  while (index < input.length) {
    const char = input[index];
    const nextChar = input[index + 1];

    if (/\s/.test(char)) {
      index += 1;
      continue;
    }

    if (char === '-' && nextChar === '-') {
      index += 2;

      while (index < input.length && input[index] !== '\n') {
        index += 1;
      }

      continue;
    }

    if (char === '/' && nextChar === '*') {
      index += 2;

      while (
        index < input.length - 1 &&
        !(input[index] === '*' && input[index + 1] === '/')
      ) {
        index += 1;
      }

      index = Math.min(index + 2, input.length);
      continue;
    }

    if (char === "'") {
      const from = index;
      index += 1;

      while (index < input.length) {
        if (input[index] === "'" && input[index + 1] === "'") {
          index += 2;
          continue;
        }

        if (input[index] === "'") {
          index += 1;
          break;
        }

        index += 1;
      }

      tokens.push({
        text: input.slice(from, index),
        upper: input.slice(from, index).toUpperCase(),
        from: offset + from,
        to: offset + index,
        kind: 'string',
      });

      continue;
    }

    if (char === '"' || char === '`' || char === '[') {
      const from = index;
      const closingChar = char === '[' ? ']' : char;

      index += 1;

      while (index < input.length) {
        if (input[index] === closingChar) {
          index += 1;
          break;
        }

        index += 1;
      }

      const raw = input.slice(from, index);
      const unquoted = raw.slice(1, -1);

      tokens.push({
        text: unquoted,
        upper: unquoted.toUpperCase(),
        from: offset + from,
        to: offset + index,
        kind: 'identifier',
      });

      continue;
    }

    if (/[A-Za-z_]/.test(char)) {
      const from = index;
      index += 1;

      while (index < input.length && /[A-Za-z0-9_$]/.test(input[index])) {
        index += 1;
      }

      const text = input.slice(from, index);

      tokens.push({
        text,
        upper: text.toUpperCase(),
        from: offset + from,
        to: offset + index,
        kind: 'keyword',
      });

      continue;
    }

    if (/[0-9]/.test(char)) {
      const from = index;
      index += 1;

      while (index < input.length && /[0-9.]/.test(input[index])) {
        index += 1;
      }

      const text = input.slice(from, index);

      tokens.push({
        text,
        upper: text.toUpperCase(),
        from: offset + from,
        to: offset + index,
        kind: 'number',
      });

      continue;
    }

    if ('(),.;'.includes(char)) {
      tokens.push({
        text: char,
        upper: char,
        from: offset + index,
        to: offset + index + 1,
        kind: 'punctuation',
      });

      index += 1;
      continue;
    }

    tokens.push({
      text: char,
      upper: char,
      from: offset + index,
      to: offset + index + 1,
      kind: 'operator',
    });

    index += 1;
  }

  return tokens;
}

function getSqlStatementTokensBeforeCursor(
  state: EditorState,
  pos: number,
): SqlToken[] {
  const maxLookBehind = 12000;
  const from = Math.max(0, pos - maxLookBehind);
  const text = state.sliceDoc(from, pos);
  const tokens = tokenizeSqlPrefix(text, from);

  let lastStatementSeparatorIndex = -1;

  for (let index = tokens.length - 1; index >= 0; index -= 1) {
    if (tokens[index].text === ';') {
      lastStatementSeparatorIndex = index;
      break;
    }
  }

  return tokens.slice(lastStatementSeparatorIndex + 1);
}

function extractUpdateTableRef(
  tokens: SqlToken[],
  updateIndex: number,
  setIndex: number,
): SqlTableRef | undefined {
  let index = updateIndex + 1;

  while (
    index < setIndex &&
    SQLITE_UPDATE_CONFLICT_TOKENS.has(tokens[index].upper)
  ) {
    index += 1;
  }

  if (tokens[index]?.upper === 'ONLY') {
    index += 1;
  }

  const parts: string[] = [];
  let expectingIdentifier = true;

  while (index < setIndex) {
    const token = tokens[index];

    if (!token) {
      break;
    }

    if (token.upper === 'AS') {
      break;
    }

    if (expectingIdentifier && isSqlIdentifierToken(token)) {
      parts.push(token.text);
      expectingIdentifier = false;
      index += 1;
      continue;
    }

    if (!expectingIdentifier && token.text === '.') {
      expectingIdentifier = true;
      index += 1;
      continue;
    }

    if (!expectingIdentifier && token.text === '*') {
      index += 1;
      continue;
    }

    break;
  }

  if (parts.length === 0) {
    return undefined;
  }

  const normalizedParts = parts.map(normalizeSqlIdentifier);
  const name = normalizedParts[normalizedParts.length - 1];
  const schema =
    normalizedParts.length > 1
      ? normalizedParts[normalizedParts.length - 2]
      : undefined;

  return {
    name,
    schema,
    fullName: normalizedParts.join('.'),
  };
}

function findTopLevelSetClauseEnd(
  tokens: SqlToken[],
  setIndex: number,
): number {
  let depth = 0;

  for (let index = setIndex + 1; index < tokens.length; index += 1) {
    const token = tokens[index];

    if (token.text === '(') {
      depth += 1;
      continue;
    }

    if (token.text === ')') {
      depth = Math.max(0, depth - 1);
      continue;
    }

    if (
      depth === 0 &&
      token.kind === 'keyword' &&
      SQL_CLAUSE_TERMINATORS_AFTER_SET.has(token.upper)
    ) {
      return index;
    }
  }

  return tokens.length;
}

function findLastTopLevelCommaIndex(
  tokens: SqlToken[],
  fromIndex: number,
  toIndex: number,
): number {
  let depth = 0;
  let lastCommaIndex = -1;

  for (let index = fromIndex; index < toIndex; index += 1) {
    const token = tokens[index];

    if (token.text === '(') {
      depth += 1;
      continue;
    }

    if (token.text === ')') {
      depth = Math.max(0, depth - 1);
      continue;
    }

    if (depth === 0 && token.text === ',') {
      lastCommaIndex = index;
    }
  }

  return lastCommaIndex;
}

function hasTopLevelEquals(tokens: SqlToken[]): boolean {
  let depth = 0;

  for (const token of tokens) {
    if (token.text === '(') {
      depth += 1;
      continue;
    }

    if (token.text === ')') {
      depth = Math.max(0, depth - 1);
      continue;
    }

    if (depth === 0 && token.text === '=') {
      return true;
    }
  }

  return false;
}

function getUpdateSetColumnContext(
  state: EditorState,
  pos: number,
): UpdateSetColumnContext | null {
  const tokens = getSqlStatementTokensBeforeCursor(state, pos);

  if (tokens.length === 0) {
    return null;
  }

  let updateIndex = -1;

  for (let index = tokens.length - 1; index >= 0; index -= 1) {
    if (tokens[index].upper === 'UPDATE') {
      updateIndex = index;
      break;
    }
  }

  if (updateIndex === -1) {
    return null;
  }

  let setIndex = -1;

  for (let index = updateIndex + 1; index < tokens.length; index += 1) {
    if (tokens[index].upper === 'SET') {
      setIndex = index;
      break;
    }
  }

  if (setIndex === -1) {
    return null;
  }

  const setClauseEndIndex = findTopLevelSetClauseEnd(tokens, setIndex);

  if (setClauseEndIndex < tokens.length) {
    return null;
  }

  const table = extractUpdateTableRef(tokens, updateIndex, setIndex);

  const lastCommaIndex = findLastTopLevelCommaIndex(
    tokens,
    setIndex + 1,
    setClauseEndIndex,
  );

  const segmentStartIndex =
    lastCommaIndex === -1 ? setIndex + 1 : lastCommaIndex + 1;

  const segmentTokens = tokens.slice(segmentStartIndex, setClauseEndIndex);

  if (hasTopLevelEquals(segmentTokens)) {
    return null;
  }

  if (segmentTokens.length === 0) {
    return {
      from: pos,
      table,
      reason: lastCommaIndex === -1 ? 'after-set' : 'after-comma',
    };
  }

  const lastToken = segmentTokens[segmentTokens.length - 1];

  if (
    segmentTokens.length === 1 &&
    isSqlIdentifierToken(lastToken) &&
    lastToken.to === pos
  ) {
    return {
      from: lastToken.from,
      table,
      reason: 'partial-column',
    };
  }

  return null;
}

function updateSetClauseAutocomplete() {
  let scheduledFrame: number | null = null;

  return EditorView.updateListener.of(update => {
    if (!update.docChanged && !update.selectionSet) {
      return;
    }

    const selection = update.state.selection.main;

    if (!selection.empty) {
      return;
    }

    if (!update.view.hasFocus) {
      return;
    }

    if (completionStatus(update.state) !== null) {
      return;
    }

    const context = getUpdateSetColumnContext(update.state, selection.head);

    if (!context) {
      return;
    }

    const shouldOpen =
      context.reason === 'after-set' ||
      context.reason === 'after-comma' ||
      update.selectionSet;

    if (!shouldOpen) {
      return;
    }

    if (scheduledFrame !== null) {
      cancelAnimationFrame(scheduledFrame);
    }

    scheduledFrame = requestAnimationFrame(() => {
      scheduledFrame = null;

      const currentSelection = update.view.state.selection.main;

      if (!currentSelection.empty) {
        return;
      }

      if (!update.view.hasFocus) {
        return;
      }

      if (completionStatus(update.view.state) !== null) {
        return;
      }

      const currentContext = getUpdateSetColumnContext(
        update.view.state,
        currentSelection.head,
      );

      if (!currentContext) {
        return;
      }

      startCompletion(update.view);
    });
  });
}

export function QueryEditor({
  value,
  onChange,
  dialect,
  schema,
  height = '360px',
  minHeight,
  maxHeight,
  theme = 'dark',
  readOnly = false,
  autoFocus = false,
  placeholderText = 'Write your query...',
  fontSize = 14,
  runMode = 'selection-or-all',
  onRun,
  className,
}: QueryEditorProps) {
  const languageExtension = useMemo(() => {
    if (dialect === 'mongo') {
      return javascript({
        jsx: false,
        typescript: false,
      });
    }

    return sql({
      dialect: getSqlDialect(dialect),
      upperCaseKeywords: true,
    });
  }, [dialect]);

  const completionExtension = useMemo(() => {
    const completionSource =
      dialect === 'mongo'
        ? createMongoCompletionSource({ schema })
        : createSqlCompletionSource({
            dialect,
            schema,
          });

    return autocompletion({
      override: [completionSource],
      activateOnTyping: true,
      maxRenderedOptions: 80,
      closeOnBlur: false,
    });
  }, [dialect, schema]);

  const runQueryKeymap = useMemo(() => {
    return Prec.highest(
      keymap.of([
        {
          key: 'Mod-Enter',
          run: view => {
            const runnableQuery = getRunnableQuery(view, runMode);

            if (!runnableQuery) {
              return true;
            }

            onRun?.(runnableQuery.query, runnableQuery.context);

            return true;
          },
        },
        {
          key: 'Shift-Mod-Enter',
          run: view => {
            const runnableQuery = getRunnableQuery(view, runMode);

            if (!runnableQuery) {
              return true;
            }

            onRun?.(runnableQuery.query, runnableQuery.context);

            return true;
          },
        },
      ]),
    );
  }, [onRun, runMode]);

  const updateSetAutocompleteExtension = useMemo(() => {
    if (dialect === 'mongo') {
      return [];
    }

    return updateSetClauseAutocomplete();
  }, [dialect]);

  const extensions = useMemo(() => {
    return [
      languageExtension,
      completionExtension,
      updateSetAutocompleteExtension,
      placeholder(placeholderText),
      runQueryKeymap,
    ];
  }, [
    languageExtension,
    completionExtension,
    updateSetAutocompleteExtension,
    placeholderText,
    runQueryKeymap,
  ]);

  return (
    <div className={className}>
      <CodeMirror
        value={value}
        height={height}
        minHeight={minHeight}
        maxHeight={maxHeight}
        theme={theme === 'dark' ? oneDark : 'light'}
        extensions={extensions}
        editable={!readOnly}
        readOnly={readOnly}
        autoFocus={autoFocus}
        basicSetup={{
          lineNumbers: true,
          foldGutter: true,
          highlightActiveLine: true,
          highlightActiveLineGutter: true,
          bracketMatching: true,
          closeBrackets: true,
          autocompletion: false,
          rectangularSelection: true,
          crosshairCursor: true,
          highlightSelectionMatches: true,
          searchKeymap: true,
          defaultKeymap: true,
          history: true,
          drawSelection: true,
          dropCursor: true,
          allowMultipleSelections: true,
          indentOnInput: true,
          syntaxHighlighting: true,
        }}
        onChange={onChange}
        style={{
          fontSize,
          borderRadius: 8,
          overflow: 'hidden',
          border: theme === 'dark' ? '1px solid #2d3340' : '1px solid #d0d7de',
        }}
      />
    </div>
  );
}

export default QueryEditor;
