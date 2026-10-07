import { useImperativeHandle, useMemo, useRef, type Ref } from 'react';
import CodeMirror, { type ReactCodeMirrorRef } from '@uiw/react-codemirror';
import {
  autocompletion,
  currentCompletions,
  snippetCompletion,
  startCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
  type CompletionSource,
} from '@codemirror/autocomplete';
import { keymap, placeholder, tooltips, EditorView } from '@codemirror/view';
import {
  EditorState,
  Prec,
  Transaction,
  type ChangeSpec,
  type Extension,
} from '@codemirror/state';
import { keywordCompletionSource, sql, PostgreSQL } from '@codemirror/lang-sql';
import { javascript } from '@codemirror/lang-javascript';
import { inkEditorTheme } from '@/lib/codemirror-theme';
import { useUiStore } from '@/stores/ui-store';
import { withClauseBoost } from './sql-completion/sql-clause-boost';
import {
  sqlStaticSource,
  staticOptionCategory,
} from './sql-completion/sql-static-sources';

const disableSpellcheck = EditorView.contentAttributes.of({
  spellcheck: 'false',
  autocorrect: 'off',
  autocapitalize: 'off',
});

/**
 * Sugestão explícita. No macOS fica em `Shift+Space` — o `Cmd+Space` é engolido pelo
 * Spotlight antes de chegar na janela. Nos outros sistemas, `Ctrl+Space`.
 *
 * Ressalva: no mac o `Shift+Space` deixa de inserir espaço. O espaço sozinho segue normal,
 * e o `Ctrl+Space` / `Alt+i` da própria lib continuam valendo como alternativa.
 */
const explicitCompletionKeymap = Prec.highest(
  keymap.of([
    {
      key: 'Mod-Space',
      mac: 'Shift-Space',
      run: startCompletion,
      preventDefault: true,
    },
  ]),
);

/**
 * Popups (autocomplete, info) montados no `body`, fora da árvore do editor. Dentro dela
 * ficam presos a qualquer ancestral com `overflow: hidden` — e, com `transform` ou
 * `backdrop-filter` no caminho (o vidro do Ink), nem o `position: fixed` escapa. Num
 * editor baixo, como o filtro WHERE, o menu simplesmente sumia. O contêiner herda as
 * classes de tema do editor, então o visual não muda.
 */
const bodyTooltips = tooltips({
  parent: typeof document === 'undefined' ? undefined : document.body,
});

/**
 * Editor de uma linha só: quebra de linha colada vira espaço. A troca é de um caractere
 * por um, então a seleção calculada pela transação original continua válida.
 */
const singleLineFilter = EditorState.transactionFilter.of(tr => {
  if (!tr.docChanged || tr.newDoc.lines === 1) return tr;

  const changes: ChangeSpec[] = [];

  tr.changes.iterChanges((from, to, _fromB, _toB, inserted) => {
    changes.push({
      from,
      to,
      insert: inserted.toString().replace(/[\r\n]/g, ' '),
    });
  });

  return {
    changes,
    selection: tr.selection,
    effects: tr.effects,
    scrollIntoView: tr.scrollIntoView,
    userEvent: tr.annotation(Transaction.userEvent),
  };
});

/**
 * Ajustes visuais do modo `singleLine`: sem respiro vertical, rolagem só horizontal.
 * `Prec.highest` porque o tema do Ink (prop `theme`) entra depois e venceria o empate.
 */
const singleLineTheme = Prec.highest(
  EditorView.theme({
    '&': { height: '100%' },
    '.cm-scroller': {
      lineHeight: '20px',
      overflowY: 'hidden',
      scrollbarWidth: 'none',
    },
    // 20 px de linha + 3 px em cima e embaixo = os 26 px úteis do Input sm.
    '.cm-content': { padding: '3px 0' },
    '.cm-line': { padding: '0' },
  }),
);

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

  /** Sem valor, segue o tema do app */
  theme?: QueryEditorTheme;
  readOnly?: boolean;
  autoFocus?: boolean;
  placeholderText?: string;

  fontSize?: number;

  /**
   * Campo de uma linha (filtro WHERE): sem gutter, Enter executa e Esc chama `onEscape`.
   * Com o popup de sugestão aberto, Enter/Esc continuam sendo dele.
   */
  singleLine?: boolean;
  onEscape?: () => void;

  runMode?: QueryEditorRunMode;
  onRun?: (query: string, context: QueryEditorRunContext) => void;

  className?: string;

  /** Handle imperativo: o botão Executar da toolbar roda o mesmo que o Mod-Enter */
  ref?: Ref<QueryEditorHandle>;
}

export interface QueryEditorHandle {
  /** Executa a seleção (ou tudo, conforme o `runMode`) — igual ao Mod-Enter */
  run: () => void;
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
  theme,
  readOnly = false,
  autoFocus = false,
  placeholderText = 'Write your query...',
  fontSize = 13,
  runMode = 'selection-or-all',
  onRun,
  singleLine = false,
  onEscape,
  className,
  ref,
}: QueryEditorProps) {
  const appTheme = useUiStore(state => state.theme);
  const resolvedTheme = theme ?? appTheme;
  const editorTheme = useMemo(
    () => inkEditorTheme(resolvedTheme === 'dark'),
    [resolvedTheme],
  );

  const codeMirrorRef = useRef<ReactCodeMirrorRef>(null);

  useImperativeHandle(
    ref,
    () => ({
      run: () => {
        const view = codeMirrorRef.current?.view;
        if (!view) return;

        const runnableQuery = getRunnableQuery(view, runMode);
        if (runnableQuery) onRun?.(runnableQuery.query, runnableQuery.context);
        // O foco volta ao editor, como depois do atalho.
        view.focus();
      },
    }),
    [onRun, runMode],
  );

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
    // Os wrappers guardam memo por cláusula, então precisam nascer e morrer junto com a
    // extensão — por isso são construídos aqui dentro.
    const sources =
      dialect === 'mongo'
        ? [createMongoCompletionSource({ schema })]
        : [
            stableSqlSource,
            withClauseBoost(
              keywordCompletionSource(PostgreSQL, true),
              'keyword',
            ),
            withClauseBoost(sqlStaticSource, staticOptionCategory),
          ];

    return autocompletion({
      override: sources,
      activateOnTyping: true,
      maxRenderedOptions: 80,
      closeOnBlur: false,
    });
  }, [dialect, schema, stableSqlSource]);

  /**
   * Cabeçalho de grupo do popup de sugestão (`<completion-section>`, elemento próprio do
   * CodeMirror). As cores vêm dos tokens do Ink, as mesmas do resto do popup
   * (lib/codemirror-theme.ts), então acompanham claro/escuro sozinhas.
   *
   * O `z-index` e o fundo opaco são o que segura o cabeçalho acima das opções ao rolar: os
   * `li` são transparentes e, sem isso, pintam por cima do cabeçalho grudado.
   *
   * O seletor repete `.cm-tooltip.cm-tooltip-autocomplete` de propósito: é exatamente o do
   * tema base, e sem empatar na especificidade o `opacity: .7` dele continuaria vencendo —
   * era o que deixava o cabeçalho translúcido.
   */
  const completionSectionTheme = useMemo(
    () =>
      EditorView.theme({
        '.cm-tooltip.cm-tooltip-autocomplete > ul > completion-section': {
          position: 'sticky',
          top: '0',
          zIndex: '1',
          backgroundColor: 'var(--surface-3)',
          borderBottom: '1px solid var(--line-subtle)',
          color: 'var(--fg-subtle)',
          opacity: '1',
          fontSize: '11px',
          fontWeight: '500',
          textTransform: 'uppercase',
          letterSpacing: '0.04em',
          padding: '6px 8px 4px',
        },
      }),
    [],
  );

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

  // Enter em `Prec.high`, abaixo do keymap do autocomplete (`Prec.highest`): com o popup
  // aberto ele aceita a sugestão; só com o popup fechado chega aqui.
  //
  // Esc não pode seguir a mesma regra: o `closeCompletion` da lib consome a tecla sempre
  // que há uma consulta pendente ou um resultado vazio, mesmo sem popup na tela — e o Esc
  // logo depois de digitar "não fazia nada". Por isso ele vai na frente do autocomplete e
  // só cede quando há opções visíveis.
  const singleLineExtensions = useMemo(() => {
    if (!singleLine) return [];

    return [
      singleLineFilter,
      singleLineTheme,
      Prec.high(
        keymap.of([
          {
            key: 'Enter',
            run: view => {
              const runnableQuery = getRunnableQuery(view, runMode);
              if (runnableQuery) {
                onRun?.(runnableQuery.query, runnableQuery.context);
              }
              return true;
            },
          },
        ]),
      ),
    ];
  }, [singleLine, onRun, runMode]);

  const escapeKeymap = useMemo(() => {
    if (!singleLine || !onEscape) return [];

    return Prec.highest(
      keymap.of([
        {
          key: 'Escape',
          run: view => {
            if (currentCompletions(view.state).length > 0) return false;
            onEscape();
            return true;
          },
        },
      ]),
    );
  }, [singleLine, onEscape]);

  const extensions = useMemo(() => {
    return [
      // Antes do `completionExtension`: no mesmo `Prec`, quem vem primeiro vence.
      escapeKeymap,
      languageExtension,
      completionExtension,
      completionSectionTheme,
      bodyTooltips,
      disableSpellcheck,
      ...(sqlExtraExtensions ? [sqlExtraExtensions] : []),
      placeholder(placeholderText),
      runQueryKeymap,
      explicitCompletionKeymap,
      ...singleLineExtensions,
    ];
  }, [
    languageExtension,
    completionExtension,
    completionSectionTheme,
    sqlExtraExtensions,
    placeholderText,
    runQueryKeymap,
    singleLineExtensions,
    escapeKeymap,
  ]);

  return (
    <div className={className}>
      <CodeMirror
        ref={codeMirrorRef}
        value={value}
        height={height}
        minHeight={minHeight}
        maxHeight={maxHeight}
        theme={editorTheme}
        extensions={extensions}
        editable={!readOnly}
        readOnly={readOnly}
        autoFocus={autoFocus}
        basicSetup={{
          lineNumbers: !singleLine,
          foldGutter: !singleLine,
          highlightActiveLine: !singleLine,
          highlightActiveLineGutter: !singleLine,
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
          overflow: 'hidden',
          // Acompanha o container (painel redimensionável) em vez de altura fixa.
          height,
        }}
      />
    </div>
  );
}

export default QueryEditor;
