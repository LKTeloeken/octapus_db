import { call } from './client';
import { RustCommand } from './commands';
import type { Server, ServerInput } from './types/server.types';

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
