import { useMemo, type FC } from 'react';
import {
  QueryEditor,
  type QueryEditorSchema,
} from '@/components/query-editor/full-sql-editor/query-editor';

import type { QueryEditorContainerProps } from './query-editor-container.types';

export const QueryEditorContainer: FC<QueryEditorContainerProps> = ({
  value,
  onChange,
  onExecute,
  databaseStructure,
}) => {
  const schema = useMemo(() => {
    if (databaseStructure) {
      const _schema: QueryEditorSchema = {
        tables: [],
      };

      for (const schema of databaseStructure.schemas) {
        for (const table of schema.tables) {
          _schema.tables?.push({
            name: table.name,
            schema: schema.name,
            columns: (table.columns || [])?.map(column => ({
              name: column.name,
              type: column.dataType,
              nullable: column.isNullable,
            })),
          });
        }
      }

      return _schema;
    }

    return undefined;
  }, [databaseStructure]);

  return (
    <div className="flex flex-col h-full rounded-xl overflow-hidden">
      <QueryEditor
        value={value}
        dialect="postgres"
        schema={schema}
        onChange={onChange}
        onRun={onExecute}
        runMode="selection-or-all"
      />
    </div>
  );
};
