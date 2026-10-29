import type { WorkspaceTab } from '@/stores/tabs-store';

type TabIdentity = Pick<
  WorkspaceTab,
  'id' | 'title' | 'serverId' | 'database' | 'schema'
>;

/** Onde a aba mora: servidor › database › schema (o schema só se houver) */
export const tabLocation = (tab: TabIdentity, serverName: string): string =>
  [serverName, tab.database, tab.schema]
    .filter((part): part is string => !!part)
    .join(' › ');

/**
 * O complemento de abas com o mesmo título (`users` em dois schemas, em dois
 * servidores…), como o DataGrip e o VS Code fazem: só as abas que colidem
 * ganham um rótulo, e ele traz apenas os níveis que variam entre elas — o
 * servidor quando é outro servidor, o schema quando é outro schema. Abas sem
 * colisão (ou que não diferem em nada) ficam de fora do mapa.
 */
export function tabDescriptions(
  tabs: TabIdentity[],
  serverName: (serverId: number) => string,
): Map<string, string> {
  const byTitle = new Map<string, TabIdentity[]>();
  for (const tab of tabs) {
    const group = byTitle.get(tab.title);
    if (group) group.push(tab);
    else byTitle.set(tab.title, [tab]);
  }

  const levels: ((tab: TabIdentity) => string)[] = [
    tab => serverName(tab.serverId),
    tab => tab.database,
    tab => tab.schema ?? '',
  ];

  const descriptions = new Map<string, string>();
  byTitle.forEach(group => {
    if (group.length < 2) return;
    const varying = levels.filter(level => new Set(group.map(level)).size > 1);
    for (const tab of group) {
      const description = varying
        .map(level => level(tab))
        .filter(Boolean)
        .join(' · ');
      if (description) descriptions.set(tab.id, description);
    }
  });
  return descriptions;
}
