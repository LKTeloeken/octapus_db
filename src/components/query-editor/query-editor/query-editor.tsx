import { useMemo, useRef } from 'react';
import CodeMirror from '@uiw/react-codemirror';
import { oneDark } from '@codemirror/theme-one-dark';
import {
  autocompletion,
  snippetCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
  type CompletionSource,
} from '@codemirror/autocomplete';
import { keymap, placeholder, EditorView } from '@codemirror/view';
import { Prec, type Extension } from '@codemirror/state';
import { keywordCompletionSource, sql, PostgreSQL } from '@codemirror/lang-sql';
import { javascript } from '@codemirror/lang-javascript';
import { sqlStaticSource } from './sql-completion/sql-static-sources';

export type QueryDialect = 'postgres' | 'mongo';

export type QueryEditorTheme = 'light' | 'dark';

export interface QueryEditorColumn {
  name: string;
  type?: string;
  nullable?: boolean;
  description?: string;
}

export interface QueryEditorCollection {
  name: string;
  fields?: QueryEditorColumn[];
  description?: string;
}

/** Metadados do dialeto Mongo. O lado SQL vem pelo `sqlCompletionSource`. */
export interface QueryEditorSchema {
  collections?: QueryEditorCollection[];
}

export interface QueryEditorProps {
  value: string;
  onChange: (value: string) => void;

  dialect: QueryDialect;
  schema?: QueryEditorSchema;

  /**
   * Autocomplete de schema do SQL, montado na camada de feature (precisa do cache de
   * queries). Deve ter identidade estável: trocar a função reconfigura o editor.
   */
  sqlCompletionSource?: CompletionSource;
  /** Extensões auxiliares do SQL (prefetch de colunas). Também precisa ser estável. */
  sqlExtraExtensions?: Extension;

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

export function QueryEditor({
  value,
  onChange,
  dialect,
  schema,
  sqlCompletionSource,
  sqlExtraExtensions,
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
  // Adaptador de identidade fixa: a prop pode oscilar sem reconfigurar o editor.
  const sqlSourceRef = useRef(sqlCompletionSource);
  sqlSourceRef.current = sqlCompletionSource;

  const stableSqlSource = useRef<CompletionSource>(
    context => sqlSourceRef.current?.(context) ?? null,
  ).current;

  const languageExtension = useMemo(() => {
    if (dialect === 'mongo') {
      return javascript({
        jsx: false,
        typescript: false,
      });
    }

    return sql({
      dialect: PostgreSQL,
      upperCaseKeywords: true,
    });
  }, [dialect]);

  const completionExtension = useMemo(() => {
    const sources =
      dialect === 'mongo'
        ? [createMongoCompletionSource({ schema })]
        : [
            stableSqlSource,
            keywordCompletionSource(PostgreSQL, true),
            sqlStaticSource,
          ];

    return autocompletion({
      override: sources,
      activateOnTyping: true,
      maxRenderedOptions: 80,
      closeOnBlur: false,
    });
  }, [dialect, schema, stableSqlSource]);

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

  const extensions = useMemo(() => {
    return [
      languageExtension,
      completionExtension,
      ...(sqlExtraExtensions ? [sqlExtraExtensions] : []),
      placeholder(placeholderText),
      runQueryKeymap,
    ];
  }, [
    languageExtension,
    completionExtension,
    sqlExtraExtensions,
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
          // Acompanha o container (painel redimensionável) em vez de altura fixa.
          height,
        }}
      />
    </div>
  );
}

export default QueryEditor;
