import { useMemo, useCallback, useEffect, useRef, type FC } from 'react';
import CodeMirror from '@uiw/react-codemirror';
import { PostgreSQL, sql } from '@codemirror/lang-sql';
import { EditorView, keymap, type ViewUpdate } from '@codemirror/view';
import { oneDark } from '@codemirror/theme-one-dark';
import { format as formatSQL } from 'sql-formatter';
import { convertToCodeMirrorSchema } from '@/shared/utils/codeMirrorAutocompleteSchema';

import type { SQLEditorProps } from './sql-editor.types';

const COLUMN_AUTOCOMPLETE_DEBOUNCE_MS = 250;

export const SQLEditor: FC<SQLEditorProps> = ({
  value,
  onChange,
  onChangeSelection,
  onRunQuery,
  className = '',
  databaseStructure,
  onRequestTableColumns,
}) => {
  const fetchTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (fetchTimeoutRef.current) {
        window.clearTimeout(fetchTimeoutRef.current);
        fetchTimeoutRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (fetchTimeoutRef.current) {
      window.clearTimeout(fetchTimeoutRef.current);
      fetchTimeoutRef.current = null;
    }
  }, [databaseStructure, onRequestTableColumns]);

  const schemaSpec = useMemo(() => {
    if (databaseStructure) {
      return convertToCodeMirrorSchema(databaseStructure);
    }
    return undefined; // Use undefined instead of {} when no schema
  }, [databaseStructure]);

  // Theme for transparent background and monospace fonts
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
      transparentDarkTheme,
      EditorView.lineWrapping,
      formatKeymap,
      runQueryKeymap,
    ],
    [transparentDarkTheme, formatKeymap, runQueryKeymap, schemaSpec],
  );

  const handleChange = useCallback(
    (next: string) => {
      onChange(next);
    },
    [onChange],
  );

  const handleUpdate = useCallback(
    (vu: ViewUpdate) => {
      if (onRequestTableColumns && databaseStructure && vu.docChanged) {
        const cursor = vu.state.selection.main.head;
        const textBeforeCursor = vu.state.sliceDoc(0, cursor);
        const match =
          // Match trailing `table.` or `schema.table.` (quoted identifiers allowed).
          /(?:(?:"?([A-Za-z0-9_]+)"?)\.)?(?:"?([A-Za-z0-9_]+)"?)\.\s*$/.exec(
            textBeforeCursor,
          );

        if (match) {
          const rawSchema = match[1];
          const rawTable = match[2];
          const tableName = rawTable?.replace(/"/g, '');
          const schemaName = rawSchema?.replace(/"/g, '');

          if (tableName) {
            const matchingSchema =
              schemaName ??
              databaseStructure.schemas.find(schema =>
                schema.tables.some(table => table.name === tableName),
              )?.name ??
              'public';
            const tableStructure = databaseStructure.schemas
              .find(schema => schema.name === matchingSchema)
              ?.tables.find(table => table.name === tableName);

            if (!tableStructure?.columns?.length) {
              if (fetchTimeoutRef.current) {
                window.clearTimeout(fetchTimeoutRef.current);
              }
              fetchTimeoutRef.current = window.setTimeout(() => {
                void onRequestTableColumns(matchingSchema, tableName);
                fetchTimeoutRef.current = null;
              }, COLUMN_AUTOCOMPLETE_DEBOUNCE_MS);
            }
          }
        }
      }

      if (!onChangeSelection) return;
      if (vu.selectionSet || vu.focusChanged) {
        const sel = vu.state.selection.main;
        onChangeSelection({ start: sel.from, end: sel.to });
      }
    },
    [databaseStructure, onChangeSelection, onRequestTableColumns],
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
        autocompletion: true,
      }}
      height="100%"
      className={`bg-transparent border-none text-white h-full ${className}`}
    />
  );
};
