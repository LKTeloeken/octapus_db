import { useMemo, useCallback, type FC } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { sql } from "@codemirror/lang-sql";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView, keymap, type ViewUpdate } from "@codemirror/view";
import { format as formatSQL } from "sql-formatter";

import type { SQLEditorProps } from "./query-editor.types";

const SQLEditor: FC<SQLEditorProps> = ({
  value,
  onChange,
  onChangeSelection,
  className = "",
}) => {
  // Tema para fundo transparente e fontes monoespaçadas
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
          },
          ".cm-scroller": { overflow: "auto" },
          ".cm-content": { caretColor: "#ffffff" },
          "&.cm-editor.cm-focused": { outline: "none" },
          ".cm-gutters": {
            backgroundColor: "transparent",
            borderRight: "none",
          },
        },
        { dark: true }
      ),
    []
  );

  // Função de formatação do SQL
  const formatAndApply = useCallback(
    (src: string) => {
      try {
        const formatted = formatSQL(src ?? "", {
          language: "sql", // ajuste para seu dialeto: 'postgresql', 'mysql', 'sqlite', etc.
          keywordCase: "upper",
          indentStyle: "standard",
        });
        if (formatted !== src) onChange(formatted);
      } catch {
        // Silencia erros de formatação para não travar a edição
      }
    },
    [onChange]
  );

  // Keybinding para formatar com Cmd/Ctrl+Shift+F
  const formatKeymap = useMemo(
    () =>
      keymap.of([
        {
          key: "Mod-Shift-f",
          run: (view) => {
            formatAndApply(view.state.doc.toString());
            return true;
          },
        },
      ]),
    [formatAndApply]
  );

  const extensions = useMemo(
    () => [
      sql(), // realce de sintaxe SQL
      oneDark,
      transparentDarkTheme,
      EditorView.lineWrapping,
      formatKeymap,
    ],
    [transparentDarkTheme, formatKeymap]
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
      // formata ao perder o foco
      // onBlur={() => formatAndApply(value)}
      theme={oneDark}
      basicSetup={{
        lineNumbers: true,
        highlightActiveLine: true,
        highlightActiveLineGutter: true,
        foldGutter: true,
      }}
      height="18rem" // equivalente ao max-h-72
      className={`bg-transparent border-none text-white max-h-full h-full ${className}`}
    />
  );
};

export default SQLEditor;
