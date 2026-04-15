import { useMemo, useCallback, useRef, type FC } from 'react';
import CodeMirror from '@uiw/react-codemirror';
import {
  PostgreSQL,
  keywordCompletionSource,
  schemaCompletionSource,
  sql,
} from '@codemirror/lang-sql';
import { EditorView, keymap, type ViewUpdate } from '@codemirror/view';
import {
  autocompletion,
  closeCompletion,
  startCompletion,
  type Completion,
  type CompletionContext,
} from '@codemirror/autocomplete';
import { oneDark } from '@codemirror/theme-one-dark';
import { format as formatSQL } from 'sql-formatter';
import { convertToCodeMirrorSchema } from '@/shared/utils/codeMirrorAutocompleteSchema';
import type { DatabaseStructure } from '@/shared/models/database.types';

import type { SQLEditorProps } from './sql-editor.types';

const IDENTIFIER_RE = '[A-Za-z0-9_]+';
const TABLE_COLUMN_CONTEXT_RE = new RegExp(
  `(?:(?:"?(${IDENTIFIER_RE})"?)\\.)?(?:"?(${IDENTIFIER_RE})"?)\\.(?:"?(${IDENTIFIER_RE})"?)?$`,
);
const UPDATE_SET_CONTEXT_RE = new RegExp(
  `\\bupdate\\s+(?:(?:"?(${IDENTIFIER_RE})"?)\\.)?(?:"?(${IDENTIFIER_RE})"?)\\s+set\\s+(?:"?(${IDENTIFIER_RE})"?)?$`,
  'i',
);
const UPDATE_TARGET_CONTEXT_RE = new RegExp(
  `\\bupdate\\s+(?:(?:"?(${IDENTIFIER_RE})"?)\\.)?(?:"?(${IDENTIFIER_RE})"?)\\s*$`,
  'i',
);

interface TableReference {
  schemaName?: string;
  tableName: string;
  columnPrefix: string;
}

const normalizeIdentifier = (identifier?: string) =>
  identifier?.replace(/"/g, '');

const parseTableColumnContext = (
  textBeforeCursor: string,
): TableReference | null => {
  const match = TABLE_COLUMN_CONTEXT_RE.exec(textBeforeCursor);
  if (!match) return null;

  const schemaName = normalizeIdentifier(match[1]);
  const tableName = normalizeIdentifier(match[2]);
  const columnPrefix = normalizeIdentifier(match[3]) ?? '';

  if (!tableName) return null;

  return { schemaName, tableName, columnPrefix };
};

const parseUpdateSetContext = (
  textBeforeCursor: string,
): TableReference | null => {
  const match = UPDATE_SET_CONTEXT_RE.exec(textBeforeCursor);
  if (!match) return null;

  const schemaName = normalizeIdentifier(match[1]);
  const tableName = normalizeIdentifier(match[2]);
  const columnPrefix = normalizeIdentifier(match[3]) ?? '';

  if (!tableName) return null;

  return { schemaName, tableName, columnPrefix };
};

const parseUpdateTargetContext = (
  textBeforeCursor: string,
): Pick<TableReference, 'schemaName' | 'tableName'> | null => {
  const match = UPDATE_TARGET_CONTEXT_RE.exec(textBeforeCursor);
  if (!match) return null;

  const schemaName = normalizeIdentifier(match[1]);
  const tableName = normalizeIdentifier(match[2]);

  if (!tableName) return null;

  return { schemaName, tableName };
};

const resolveTable = (
  structure: DatabaseStructure,
  schemaName: string | undefined,
  tableName: string,
) => {
  if (schemaName) {
    const schema = structure.schemas.find(item => item.name === schemaName);
    if (!schema) return null;

    const table = schema.tables.find(item => item.name === tableName);
    return table ? { schemaName, table } : null;
  }

  const schemaWithTable = structure.schemas.find(schema =>
    schema.tables.some(table => table.name === tableName),
  );

  if (!schemaWithTable) return null;

  const table = schemaWithTable.tables.find(item => item.name === tableName);
  return table ? { schemaName: schemaWithTable.name, table } : null;
};

const buildTableCompletions = (structure: DatabaseStructure): Completion[] => {
  const options: Completion[] = [];

  for (const schema of structure.schemas) {
    options.push({
      label: schema.name,
      type: 'namespace',
      detail: 'schema',
      boost: 40,
    });

    for (const table of schema.tables) {
      options.push({
        label: `${schema.name}.${table.name}`,
        type: 'class',
        detail: `${table.tableType} table`,
        boost: 80,
      });

      if (schema.name === 'public') {
        options.push({
          label: table.name,
          type: 'class',
          detail: `${table.tableType} table`,
          boost: 90,
        });
      }
    }
  }

  return options;
};

export const SQLEditor: FC<SQLEditorProps> = ({
  value,
  onChange,
  onChangeSelection,
  onRunQuery,
  className = '',
  databaseStructure,
  onRequestTableColumns,
}) => {
  const editorViewRef = useRef<EditorView | null>(null);
  const loadingColumnsRef = useRef<Set<string>>(new Set());

  const schemaSpec = useMemo(() => {
    if (databaseStructure) {
      return convertToCodeMirrorSchema(databaseStructure);
    }
    return undefined;
  }, [databaseStructure]);

  const tableCompletions = useMemo(
    () => (databaseStructure ? buildTableCompletions(databaseStructure) : []),
    [databaseStructure],
  );

  const requestTableColumns = useCallback(
    (schemaName: string, tableName: string) => {
      if (!onRequestTableColumns) return;

      const requestKey = `${schemaName}.${tableName}`;
      if (loadingColumnsRef.current.has(requestKey)) return;

      loadingColumnsRef.current.add(requestKey);
      void onRequestTableColumns(schemaName, tableName).finally(() => {
        loadingColumnsRef.current.delete(requestKey);
        const view = editorViewRef.current;
        if (!view) return;

        const reopenCompletion = () => {
          closeCompletion(view);
          startCompletion(view);
        };

        reopenCompletion();
        window.setTimeout(reopenCompletion, 80);
        window.setTimeout(reopenCompletion, 180);
      });
    },
    [onRequestTableColumns],
  );

  const contextCompletionSource = useCallback(
    (context: CompletionContext) => {
      if (!databaseStructure) return null;

      const textBeforeCursor = context.state.sliceDoc(0, context.pos);
      const tableContext = parseTableColumnContext(textBeforeCursor);
      const updateSetContext = parseUpdateSetContext(textBeforeCursor);

      if (tableContext) {
        const resolvedTable = resolveTable(
          databaseStructure,
          tableContext.schemaName,
          tableContext.tableName,
        );

        if (!resolvedTable) return null;

        const requestKey = `${resolvedTable.schemaName}.${resolvedTable.table.name}`;
        const from = context.pos - tableContext.columnPrefix.length;

        if (!resolvedTable.table.columns?.length) {
          requestTableColumns(
            resolvedTable.schemaName,
            resolvedTable.table.name,
          );
          return {
            from,
            options: [
              {
                label: loadingColumnsRef.current.has(requestKey)
                  ? 'Loading columns...'
                  : 'No columns cached yet',
                type: 'keyword',
                detail: `${resolvedTable.schemaName}.${resolvedTable.table.name}`,
                apply: () => {},
              },
            ],
          };
        }

        return {
          from,
          options: resolvedTable.table.columns.map(column => ({
            label: column.name,
            type: 'property',
            detail: column.dataType,
            boost: 100,
          })),
          validFor: /^"?[A-Za-z0-9_]*"?$/,
        };
      }

      if (updateSetContext) {
        const resolvedTable = resolveTable(
          databaseStructure,
          updateSetContext.schemaName,
          updateSetContext.tableName,
        );

        if (!resolvedTable) return null;

        const requestKey = `${resolvedTable.schemaName}.${resolvedTable.table.name}`;
        const from = context.pos - updateSetContext.columnPrefix.length;

        if (!resolvedTable.table.columns?.length) {
          requestTableColumns(
            resolvedTable.schemaName,
            resolvedTable.table.name,
          );
          return {
            from,
            options: [
              {
                label: loadingColumnsRef.current.has(requestKey)
                  ? 'Loading columns...'
                  : 'No columns cached yet',
                type: 'keyword',
                detail: `${resolvedTable.schemaName}.${resolvedTable.table.name}`,
                apply: () => {},
              },
            ],
          };
        }

        return {
          from,
          options: resolvedTable.table.columns.map(column => ({
            label: column.name,
            type: 'property',
            detail: column.dataType,
            boost: 100,
          })),
          validFor: /^"?[A-Za-z0-9_]*"?$/,
        };
      }

      const word = context.matchBefore(/"?[A-Za-z0-9_]*"?/);
      if (!word) return null;
      if (word.from === word.to && !context.explicit) return null;

      const beforeWord = context.state.sliceDoc(0, word.from);
      const shouldSuggestTables =
        /\b(from|join|update|into|table|delete\s+from)\s*$/i.test(beforeWord) ||
        context.explicit;

      if (!shouldSuggestTables || tableCompletions.length === 0) return null;

      return {
        from: word.from,
        options: tableCompletions,
        validFor: /^"?[A-Za-z0-9_.]*"?$/,
      };
    },
    [databaseStructure, requestTableColumns, tableCompletions],
  );

  const transparentDarkTheme = useMemo(
    () =>
      EditorView.theme(
        {
          '&': {
            backgroundColor: 'transparent',
            color: 'white',
            fontFamily:
              'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, monospace',
            fontSize: '14px',
            borderRadius: '8px !important',
          },
          '.cm-scroller': { overflow: 'auto' },
          '.cm-content': { caretColor: '#ffffff' },
          '&.cm-editor.cm-focused': { outline: 'none', border: 'none' },
          '.cm-gutters': {
            backgroundColor: 'transparent',
            border: 'none !important',
          },
        },
        { dark: true },
      ),
    [],
  );

  const formatAndApply = useCallback(
    (src: string) => {
      try {
        const formatted = formatSQL(src ?? '', {
          language: 'sql',
          keywordCase: 'upper',
          indentStyle: 'standard',
        });
        if (formatted !== src) onChange(formatted);
      } catch {
        // Silently ignore formatting errors
      }
    },
    [onChange],
  );

  const formatKeymap = useMemo(
    () =>
      keymap.of([
        {
          mac: 'Mod-Shift-f',
          linux: 'Ctrl-Shift-f',
          win: 'Ctrl-Shift-f',
          run: view => {
            formatAndApply(view.state.doc.toString());
            return true;
          },
        },
      ]),
    [formatAndApply],
  );

  const runQueryKeymap = useMemo(
    () =>
      keymap.of([
        {
          mac: 'Mod-Shift-Enter',
          linux: 'Ctrl-Enter',
          win: 'Ctrl-Enter',
          run: () => {
            onRunQuery?.();
            return true;
          },
        },
      ]),
    [onRunQuery],
  );

  const extensions = useMemo(
    () => [
      sql({
        dialect: PostgreSQL,
        schema: schemaSpec,
        upperCaseKeywords: true,
      }),
      autocompletion({
        activateOnTyping: true,
        maxRenderedOptions: 200,
        override: [
          contextCompletionSource,
          schemaCompletionSource({
            dialect: PostgreSQL,
            schema: schemaSpec,
            upperCaseKeywords: true,
          }),
          keywordCompletionSource(PostgreSQL, true),
        ],
      }),
      transparentDarkTheme,
      EditorView.lineWrapping,
      formatKeymap,
      runQueryKeymap,
    ],
    [
      contextCompletionSource,
      transparentDarkTheme,
      formatKeymap,
      runQueryKeymap,
      schemaSpec,
    ],
  );

  const handleChange = useCallback(
    (next: string) => {
      onChange(next);
    },
    [onChange],
  );

  const handleUpdate = useCallback(
    (vu: ViewUpdate) => {
      if (databaseStructure && vu.docChanged) {
        const cursor = vu.state.selection.main.head;
        const textBeforeCursor = vu.state.sliceDoc(0, cursor);
        const updateTargetContext = parseUpdateTargetContext(textBeforeCursor);

        if (updateTargetContext) {
          const resolvedTable = resolveTable(
            databaseStructure,
            updateTargetContext.schemaName,
            updateTargetContext.tableName,
          );

          if (resolvedTable && !resolvedTable.table.columns?.length) {
            requestTableColumns(
              resolvedTable.schemaName,
              resolvedTable.table.name,
            );
          }
        }
      }

      if (!onChangeSelection) return;
      if (vu.selectionSet || vu.focusChanged) {
        const sel = vu.state.selection.main;
        onChangeSelection({ start: sel.from, end: sel.to });
      }
    },
    [databaseStructure, onChangeSelection, requestTableColumns],
  );

  return (
    <CodeMirror
      value={value}
      onChange={handleChange}
      onUpdate={handleUpdate}
      extensions={extensions}
      theme={oneDark}
      basicSetup={{
        lineNumbers: true,
        highlightActiveLine: true,
        highlightActiveLineGutter: true,
        foldGutter: true,
        autocompletion: false,
      }}
      onCreateEditor={editor => {
        editorViewRef.current = editor;
      }}
      height="100%"
      className={`bg-transparent border-none text-white h-full ${className}`}
    />
  );
};
