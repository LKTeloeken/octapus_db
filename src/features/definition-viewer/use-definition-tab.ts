import { useQuery } from '@tanstack/react-query';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import toast from 'react-hot-toast';
import type { CatalogObjectKind } from '@/api/types/catalog.types';
import { errorDetails } from '@/lib/error-details';
import { useDbCapabilities } from '@/queries/use-capabilities';
import { catalogDefinitionQuery } from '@/queries/use-catalog';
import { useServers } from '@/queries/use-servers';
import { useTabsStore, type DefinitionTab } from '@/stores/tabs-store';

/** O que o objeto é, no cabeçalho da aba */
export const OBJECT_KIND_LABELS: Record<CatalogObjectKind, string> = {
  view: 'View',
  materializedView: 'View materializada',
  index: 'Índice',
  function: 'Função',
  sequence: 'Sequence',
  trigger: 'Trigger',
  foreignKey: 'Chave estrangeira',
};

/**
 * A aba de definição: o texto vem do catálogo (`catalog_object_definition`),
 * só leitura. "Abrir em aba de query" leva o texto para uma aba de query nova,
 * onde dá para editar e executar.
 */
export const useDefinitionTab = (tab: DefinitionTab) => {
  const definition = useQuery(
    catalogDefinitionQuery(tab.serverId, tab.database, tab.target),
  );
  const { data: servers } = useServers();
  const capsByType = useDbCapabilities();
  const openQueryTab = useTabsStore(state => state.openQueryTab);

  const server = servers?.find(item => item.id === tab.serverId);
  const dbType = server?.dbType;
  // Sem as capabilities ainda, SQL (o caso comum); o Mongo é o shell (JS)
  const supportsSql = dbType ? (capsByType[dbType]?.supportsSql ?? true) : true;

  const text = definition.data ?? '';

  const openInQuery = () => {
    openQueryTab({
      serverId: tab.serverId,
      database: tab.database,
      schema: tab.schema,
      title: tab.title,
      content: text,
    });
  };

  const copy = async () => {
    try {
      await writeText(text);
      toast.success('Definição copiada.');
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return {
    text,
    isLoading: definition.isPending,
    error: definition.isError ? errorDetails(definition.error) : null,
    retry: () => void definition.refetch(),
    kindLabel: OBJECT_KIND_LABELS[tab.target.kind],
    serverName: server?.name ?? '',
    dbType,
    dialect: supportsSql ? ('postgres' as const) : ('mongo' as const),
    openInQuery,
    copy,
  };
};
