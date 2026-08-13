import { useEffect, useRef } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { CompletionSource } from '@codemirror/autocomplete';
import type { Extension } from '@codemirror/state';
import { listColumns } from '@/api/structure';
import type {
  ColumnInfo,
  DatabaseStructure,
} from '@/api/types/structure.types';
import {
  createSqlSchemaSource,
  sqlPrefetchListener,
} from '@/components/query-editor/query-editor/sql-completion/sql-schema-source';
import type { SqlCompletionPorts } from '@/components/query-editor/query-editor/sql-completion/sql-completion.types';
import { STRUCTURE_STALE_TIME_MS } from '@/providers/query-provider';
import { queryKeys } from '@/queries/keys';
import { useStructure } from '@/queries/use-structure';

export interface UseSqlCompletionParams {
  serverId: number;
  database: string;
  enabled: boolean;
}

export interface SqlCompletion {
  source: CompletionSource;
  prefetch: Extension;
}

/**
 * Autocomplete SQL do editor: estrutura do banco + colunas buscadas sob demanda.
 *
 * As colunas vivem na mesma key (`queryKeys.columns`) que a árvore de conexões preenche,
 * e esse cache é persistido em disco — expandir a tabela na árvore aquece o editor e
 * vice-versa, e depois de um restart não há request nenhum.
 *
 * A `CompletionSource` devolvida tem identidade fixa de propósito: ela é lida pelo
 * CodeMirror dentro de uma extensão, e trocar a função obrigaria a reconfigurar o editor
 * (o popup aberto se perderia). Por isso o alvo (`serverId`/`database`/estrutura) mora
 * num ref lido na hora da chamada — a aba pode trocar de banco sem remontar o painel.
 */
export function useSqlCompletion({
  serverId,
  database,
  enabled,
}: UseSqlCompletionParams): SqlCompletion {
  const queryClient = useQueryClient();
  const { data: structure } = useStructure(serverId, database, { enabled });

  const target = useRef<{
    serverId: number;
    database: string;
    structure: DatabaseStructure | undefined;
  }>({ serverId, database, structure });

  target.current = { serverId, database, structure };

  const columnsVersion = useRef(0);
  const ports = useRef<SqlCompletionPorts | null>(null);

  if (!ports.current) {
    ports.current = {
      getStructure: () => target.current.structure,

      peekColumns: (schema, table) =>
        queryClient.getQueryData<ColumnInfo[]>(
          queryKeys.columns(
            target.current.serverId,
            target.current.database,
            schema,
            table,
          ),
        ),

      ensureColumns: async (schema, table) => {
        const { serverId: id, database: db } = target.current;

        try {
          // Devolve o cache quando existe e só busca quando falta; chamadas simultâneas
          // para a mesma tabela compartilham a mesma promise.
          return await queryClient.ensureQueryData({
            queryKey: queryKeys.columns(id, db, schema, table),
            queryFn: () => listColumns(id, db, schema, table),
            staleTime: STRUCTURE_STALE_TIME_MS,
            gcTime: STRUCTURE_STALE_TIME_MS,
          });
        } catch {
          // Tabela sem permissão ou removida: o resto do autocomplete segue funcionando.
          return undefined;
        }
      },

      getColumnsVersion: () => columnsVersion.current,
    };
  }

  // Colunas podem chegar por fora (árvore de conexões). O contador invalida o namespace
  // já compilado pelo lang-sql, que não enxerga mutação depois de montado.
  useEffect(() => {
    return queryClient.getQueryCache().subscribe(event => {
      if (
        event.type === 'updated' &&
        event.action.type === 'success' &&
        event.query.queryKey[0] === 'columns'
      ) {
        columnsVersion.current += 1;
      }
    });
  }, [queryClient]);

  const completion = useRef<SqlCompletion | null>(null);

  if (!completion.current) {
    completion.current = {
      source: createSqlSchemaSource(ports.current),
      prefetch: sqlPrefetchListener(ports.current),
    };
  }

  return completion.current;
}
