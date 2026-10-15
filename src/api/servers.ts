import { call } from './client';
import { RustCommand } from './commands';
import type { Server, ServerGroup, ServerInput } from './types/server.types';

export function getAllServers(): Promise<Server[]> {
  return call<Server[]>(RustCommand.GetAllServers);
}

export function getServer(id: number): Promise<Server> {
  return call<Server>(RustCommand.GetServer, { id });
}

export function createServer(input: ServerInput): Promise<Server> {
  return call<Server>(RustCommand.CreateServer, { input });
}

/** Backend drops the server's open connections on update */
export function updateServer(id: number, input: ServerInput): Promise<Server> {
  return call<Server>(RustCommand.UpdateServer, { id, input });
}

/** Deletes keychain password and open connections too */
export function deleteServer(id: number): Promise<void> {
  return call<void>(RustCommand.DeleteServer, { id });
}

/** Move o servidor para um grupo (null = fora de grupo); a senha não muda */
export function setServerGroup(
  id: number,
  groupId: number | null,
): Promise<void> {
  return call<void>(RustCommand.SetServerGroup, { id, groupId });
}

export function getAllServerGroups(): Promise<ServerGroup[]> {
  return call<ServerGroup[]>(RustCommand.GetAllServerGroups);
}

export function createServerGroup(name: string): Promise<ServerGroup> {
  return call<ServerGroup>(RustCommand.CreateServerGroup, { name });
}

export function renameServerGroup(
  id: number,
  name: string,
): Promise<ServerGroup> {
  return call<ServerGroup>(RustCommand.RenameServerGroup, { id, name });
}

/** Os servidores do grupo ficam fora de grupo — não são excluídos */
export function deleteServerGroup(id: number): Promise<void> {
  return call<void>(RustCommand.DeleteServerGroup, { id });
}
