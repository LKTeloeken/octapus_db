import { useRef, useEffect, useCallback, type FC } from "react";
import Editor from "react-simple-code-editor";
import Prism from "prismjs";
import "prismjs/components/prism-sql";
import "./prism.css"; // ajustar tema conforme quiser

import type { SQLEditorProps } from "./query-editor.types";

const SQLEditor: FC<SQLEditorProps> = ({
  value,
  onChange,
  onChangeSelection,
  className = "",
  customRef,
}) => {
  const editorRef = useRef<HTMLTextAreaElement | null>(null);

  // Callback para destacar sintaxe
  const highlight = useCallback((code: string) => {
    return Prism.highlight(code, Prism.languages.sql, "sql");
  }, []);

  // Efeito para ouvir seleção se tiver o onChangeSelection
  useEffect(() => {
    if (!onChangeSelection) return;

    const textarea = editorRef.current;
    if (!textarea) return;

    const handleSelect = () => {
      onChangeSelection({
        start: textarea.selectionStart,
        end: textarea.selectionEnd,
      });
    };

    textarea.addEventListener("select", handleSelect);
    return () => {
      textarea.removeEventListener("select", handleSelect);
    };
  }, [onChangeSelection]);

  return (
    <Editor
      ref={customRef}
      value={value}
      onValueChange={onChange}
      highlight={highlight}
      padding={10}
      textareaId="sql-editor-textarea"
      style={{
        fontFamily: "monospace",
        fontSize: 14,
        outline: "none",
      }}
      className="bg-transparent border-none text-white overflow-scroll max-h-72"
    />
  );
};

export default SQLEditor;
