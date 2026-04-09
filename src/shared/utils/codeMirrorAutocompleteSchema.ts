import type { DatabaseStructure } from '@/shared/models/database.types';

export interface SQLSchemaSpec {
  [schemaOrTable: string]:
    | readonly string[]
    | { [table: string]: readonly string[] };
}

export function convertToCodeMirrorSchema(
  structure: DatabaseStructure,
  defaultSchema = 'public',
): SQLSchemaSpec {
  const schema: SQLSchemaSpec = {};

  for (const schemaInfo of structure.schemas) {
    const tables: { [table: string]: readonly string[] } = {};

    for (const table of schemaInfo.tables) {
      const columnNames = table.columns?.map(col => col.name) || [];
      tables[table.name] = columnNames;

      // Also add fully qualified name for non-default schemas
      if (schemaInfo.name !== defaultSchema) {
        schema[`${schemaInfo.name}.${table.name}`] = columnNames;
      }
    }

    // Add schema with its tables
    schema[schemaInfo.name] = tables;

    // For default schema, also add tables at root level
    if (schemaInfo.name === defaultSchema) {
      Object.assign(schema, tables);
    }
  }

  return schema;
}

export function getFlatCompletions(structure: DatabaseStructure) {
  const completions: Array<{
    label: string;
    type: 'schema' | 'table' | 'column' | 'keyword';
    detail?: string;
  }> = [];

  for (const schema of structure.schemas) {
    completions.push({ label: schema.name, type: 'schema' });

    for (const table of schema.tables) {
      completions.push({
        label: table.name,
        type: 'table',
        detail: `${schema.name} - ${table.tableType}`,
      });

      if (!table.columns) continue;

      for (const column of table.columns) {
        completions.push({
          label: column.name,
          type: 'column',
          detail: `${table.name}.${column.name} (${column.dataType})`,
        });
      }
    }
  }

  return completions;
}
