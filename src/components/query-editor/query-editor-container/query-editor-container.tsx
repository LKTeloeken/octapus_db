import { useCallback, type FC } from "react";
import { format as formatSQL } from "sql-formatter";
import { SQLEditor } from "@/components/query-editor/sql-editor/sql-editor";
import { QueryEditorToolbar } from "@/components/query-editor/query-editor-toolbar/query-editor-toolbar";

import type { QueryEditorContainerProps } from "./query-editor-container.types";
import type { OnSelectionChange } from "@/components/layout/query-editor/query-editor.types";

export const QueryEditorContainer: FC<QueryEditorContainerProps> = ({
  value,
  onChange,
  onChangeSelection,
  onExecute,
  databaseStructure,
  isLoading = false,
}) => {
  const handleFormat = useCallback(() => {
    try {
      const formatted = formatSQL(value ?? "", {
        language: "sql",
        keywordCase: "upper",
        indentStyle: "standard",
      });
      if (formatted !== value) onChange(formatted);
    } catch {
      // Silently ignore formatting errors
    }
  }, [value, onChange]);

  const handleSelectionChange: OnSelectionChange = useCallback(
    (selection) => {
      if (onChangeSelection) {
        const query = value.slice(selection.start, selection.end);
        onChangeSelection(query);
      }
    },
    [onChangeSelection, value]
  );

  const handleRun = useCallback(() => {
    onExecute();
  }, [onExecute]);

  return (
    <div className="flex flex-col h-full">
      <QueryEditorToolbar
        onRun={handleRun}
        onFormat={handleFormat}
        isLoading={isLoading}
        disabled={!value.trim()}
      />
      <div className="flex-1 rounded-xl overflow-hidden">
        <SQLEditor
          value={value}
          databaseStructure={databaseStructure}
          onChange={onChange}
          onChangeSelection={handleSelectionChange}
          onRunQuery={handleRun}
        />
      </div>
    </div>
  );
};
