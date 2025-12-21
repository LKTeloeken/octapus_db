import { useMemo, useCallback, type FC } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { sql } from "@codemirror/lang-sql";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView, keymap, type ViewUpdate } from "@codemirror/view";
import { format as formatSQL } from "sql-formatter";

import type { SQLEditorProps } from "./sql-editor.types";

export const SQLEditor: FC<SQLEditorProps> = ({
  value,
  onChange,
  onChangeSelection,
  onRunQuery,
  className = "",
}) => {
  // Theme for transparent background and monospace fonts
  const transparentDarkTheme = useMemo(
    () =>
      EditorView.theme(
        {
          "&": {
            backgroundColor: "transparent",
            color: "white",
            fontFamily:
              "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, monospace",
            fontSize: "14px",
            borderRadius: "8px !important",
          },
          ".cm-scroller": { overflow: "auto" },
          ".cm-content": { caretColor: "#ffffff" },
          "&.cm-editor.cm-focused": { outline: "none", border: "none" },
          ".cm-gutters": {
            backgroundColor: "transparent",
            border: "none !important",
          },
        },
        { dark: true }
      ),
    []
  );

  // SQL formatting function
  const formatAndApply = useCallback(
    (src: string) => {
      try {
        const formatted = formatSQL(src ?? "", {
          language: "sql",
          keywordCase: "upper",
          indentStyle: "standard",
        });
        if (formatted !== src) onChange(formatted);
      } catch {
        // Silently ignore formatting errors
      }
    },
    [onChange]
  );

  // Keybinding for formatting with Cmd/Ctrl+Shift+F
  const formatKeymap = useMemo(
    () =>
      keymap.of([
        {
          mac: "Mod-Shift-f",
          linux: "Ctrl-Shift-f",
          win: "Ctrl-Shift-f",
          run: (view) => {
            formatAndApply(view.state.doc.toString());
            return true;
          },
        },
      ]),
    [formatAndApply]
  );

  const runQueryKeymap = useMemo(
    () =>
      keymap.of([
        {
          mac: "Mod-Shift-Enter",
          linux: "Ctrl-Enter",
          win: "Ctrl-Enter",
          run: (view) => {
            onRunQuery?.();
            return true;
          },
        },
      ]),
    [onRunQuery]
  );

  const extensions = useMemo(
    () => [
      sql(),
      oneDark,
      transparentDarkTheme,
      EditorView.lineWrapping,
      formatKeymap,
      runQueryKeymap,
    ],
    [transparentDarkTheme, formatKeymap, runQueryKeymap]
  );

  const handleChange = useCallback(
    (next: string) => {
      onChange(next);
    },
    [onChange]
  );

  const handleUpdate = useCallback(
    (vu: ViewUpdate) => {
      if (!onChangeSelection) return;
      if (vu.selectionSet || vu.focusChanged) {
        const sel = vu.state.selection.main;
        onChangeSelection({ start: sel.from, end: sel.to });
      }
    },
    [onChangeSelection]
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
      }}
      height="100%"
      className={`bg-transparent border-none text-white h-full ${className}`}
    />
  );
};
