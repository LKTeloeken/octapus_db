import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { applyStructureChange, previewStructureChange } from '@/api/structure';
import { FLAT_SCHEMA } from '@/api/types/catalog.types';
import type { StructureChange } from '@/api/types/structure.types';
import { queryKeys } from './keys';

export interface StructureChangeVariables {
  serverId: number;
  database: string;
  change: StructureChange;
}

/**
 * O SQL que a mudança vai rodar, para o diálogo mostrar antes de aplicar. O
 * chamador passa a mudança já com debounce: cada versão é uma key. Sem
 * `change` (formulário incompleto) não vai ao backend.
 */
export function useStructureChangePreview(
  serverId: number,
  database: string,
  change: StructureChange | null,
) {
  return useQuery({
    queryKey: queryKeys.structurePreview(serverId, database, change),
    queryFn: () => previewStructureChange(serverId, database, change!),
    enabled: change !== null,
    // Enquanto a prévia nova não chega, a anterior fica (sem piscar)
    placeholderData: keepPreviousData,
    retry: false,
    staleTime: 0,
    gcTime: 30_000,
  });
}

/**
 * Cria, renomeia ou remove uma tabela ou coluna. O backend recarrega o schema
 * no catálogo e o `catalog-event` atualiza a árvore; aqui saem as colunas, os
 * índices e os dados em cache da tabela (se ela continua existindo com o mesmo
 * nome), e a comparação entre schemas.
 */
export function useApplyStructureChange() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ serverId, database, change }: StructureChangeVariables) =>
      applyStructureChange(serverId, database, change),
    onSuccess: (_result, { serverId, database, change }) => {
      const schema = change.schema ?? FLAT_SCHEMA;
      const { table } = change;
      const comparison = queryClient.invalidateQueries({
        queryKey: queryKeys.schemaComparisonScope(serverId, database),
      });
      // A tabela deixou de existir com esse nome: refazer as consultas dela só
      // mostraria erro na aba (que a tela fecha)
      if (change.type === 'dropTable' || change.type === 'renameTable') {
        return comparison;
      }
      return Promise.all([
        comparison,
        queryClient.invalidateQueries({
          queryKey: queryKeys.columns(serverId, database, schema, table),
        }),
        queryClient.invalidateQueries({
          queryKey: queryKeys.indexes(serverId, database, schema, table),
        }),
        queryClient.invalidateQueries({
          queryKey: queryKeys.tableDataForTable(
            serverId,
            database,
            change.schema,
            table,
          ),
        }),
      ]);
    },
  });
}
