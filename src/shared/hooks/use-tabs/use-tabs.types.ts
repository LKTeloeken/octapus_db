export type OpenTab = (serverId: number, databaseName: string) => void;
export type CloseTab = (id: string) => void;
export type SetActiveTabId = (id: string) => void;
export type SetTabContent = (id: string, content: string) => void;
