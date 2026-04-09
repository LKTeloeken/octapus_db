import { invoke } from '../utils/invokeHandler';
import { RustMethods } from '../rust-functions';
import type { Server, ServerPrimitive } from '@/shared/models/servers.types';

export async function createServer(server: ServerPrimitive): Promise<Server> {
  return await invoke<Server>(RustMethods.CREATE_SERVER, {
    input: { ...server, dbType: 'postgres' },
  });
}

export async function getAllServers(): Promise<Server[]> {
  return await invoke<Server[]>(RustMethods.GET_ALL_SERVERS, {});
}

export async function getServerById(id: number): Promise<Server> {
  return await invoke<Server>(RustMethods.GET_SERVER_BY_ID, { id });
}

export async function updateServer(newServer: Server): Promise<Server> {
  return await invoke<Server>(RustMethods.UPDATE_SERVER, { ...newServer });
}

export async function deleteServer(id: number): Promise<void> {
  return await invoke<void>(RustMethods.DELETE_SERVER, { id });
}

export async function ensureServerConnection(
  serverId: number,
  database?: string,
): Promise<boolean> {
  return await invoke<boolean>(RustMethods.CONNECT_TO_SERVER, {
    serverId,
    database,
  });
}
