import { useCallback, useMemo } from 'react';
import { useDbCapabilities } from '@/queries/use-capabilities';
import { useDatabases } from '@/queries/use-databases';
import { useServers } from '@/queries/use-servers';
import {
  useAssistantStore,
  type ConversationTab,
} from '@/stores/assistant-store';

/**
 * Onde a conversa pergunta (C-01, RN-17): só servidores cujo banco o
 * assistente atende (`supportsAssistant`). Tipo sem capabilities ainda =
 * "não sei": fica de fora até a tabela chegar, sem ser tratado como recusa.
 * Conversa já aberta fica presa ao paradigma dela.
 */
export const useTargetPicker = ({
  tab,
  paradigm,
  disabled,
}: {
  tab: ConversationTab;
  /** O paradigma da conversa no servidor, quando ela já existe */
  paradigm: string | null;
  disabled: boolean;
}) => {
  const { data: servers, isPending: serversLoading } = useServers();
  const capsByType = useDbCapabilities();
  const setTarget = useAssistantStore(state => state.setTarget);
  const target = tab.target;

  const options = useMemo(
    () =>
      (servers ?? []).filter(
        server =>
          capsByType[server.dbType]?.supportsAssistant === true &&
          (paradigm === null || server.dbType === paradigm),
      ),
    [servers, capsByType, paradigm],
  );
  const capsLoading = Object.keys(capsByType).length === 0;

  const server = options.find(item => item.id === target?.serverId) ?? null;
  const databases = useDatabases(server?.id);

  const selectServer = useCallback(
    (value: string) => {
      const next = options.find(item => String(item.id) === value);
      if (!next) return;
      setTarget(tab.id, {
        serverId: next.id,
        database: next.defaultDatabase ?? '',
        schema: null,
      });
    },
    [options, setTarget, tab.id],
  );

  const selectDatabase = useCallback(
    (database: string) => {
      if (!target) return;
      setTarget(tab.id, { ...target, database, schema: null });
    },
    [setTarget, tab.id, target],
  );

  const databaseNames = useMemo(() => {
    const names = (databases.data ?? []).map(db => db.name);
    // O database do alvo aparece mesmo que a lista ainda não tenha chegado
    if (target?.database && !names.includes(target.database)) {
      names.unshift(target.database);
    }
    return names;
  }, [databases.data, target?.database]);

  return {
    servers: options,
    loading: serversLoading || capsLoading,
    serverValue: server ? String(server.id) : '',
    selectServer,
    server,
    databases: databaseNames,
    databaseValue: target?.database ?? '',
    selectDatabase,
    databasesLoading: databases.isPending && !!server,
    schema: target?.schema ?? null,
    disabled,
  };
};
