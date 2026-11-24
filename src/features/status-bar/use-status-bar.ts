import { useEffect, useMemo, useState } from 'react';
import { useServerGroups } from '@/queries/use-server-groups';
import { useServers } from '@/queries/use-servers';
import { useActivityStore } from '@/stores/activity-store';
import { useTabsStore } from '@/stores/tabs-store';
import { useUiStore } from '@/stores/ui-store';

/** Grupo com "prod" no nome: a conexão é tratada como produção */
const PRODUCTION_GROUP = /\bprod/i;

/** Segundos desde que a consulta partiu, atualizado a cada 100 ms */
function useElapsed(running: boolean) {
  const [startedAt, setStartedAt] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!running) {
      setStartedAt(null);
      return;
    }
    const start = Date.now();
    setStartedAt(start);
    setNow(start);
    const timer = window.setInterval(() => setNow(Date.now()), 100);
    return () => window.clearInterval(timer);
  }, [running]);
  return startedAt === null ? null : (now - startedAt) / 1000;
}

/**
 * O que a barra de status mostra (DESIGN.md, Barra de status): a conexão da
 * aba ativa e se algo está rodando.
 */
export const useStatusBar = () => {
  const activeTab = useTabsStore(state =>
    state.activeTabId ? state.tabs.get(state.activeTabId) : undefined,
  );
  const { data: servers } = useServers();
  const { data: groups } = useServerGroups();
  const runningTabs = useActivityStore(state => state.runningTabs);
  const mascot = useUiStore(state => state.mascot);

  const server = useMemo(
    () => servers?.find(item => item.id === activeTab?.serverId),
    [servers, activeTab?.serverId],
  );
  const group = useMemo(
    () => groups?.find(item => item.id === server?.groupId),
    [groups, server?.groupId],
  );

  const running = activeTab ? runningTabs.has(activeTab.id) : false;
  const elapsed = useElapsed(running);

  return {
    connection:
      activeTab && server
        ? {
            serverName: server.name,
            database: activeTab.database,
            dbType: server.dbType,
            production: !!group && PRODUCTION_GROUP.test(group.name),
          }
        : null,
    running,
    /** Alguma aba (não só a ativa) roda: o polvo trabalha */
    anyRunning: runningTabs.size > 0,
    elapsed,
    mascot,
  };
};
