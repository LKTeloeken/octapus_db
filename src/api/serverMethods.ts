import { invoke } from "./utils/invokeHandler";
import { IServer, IServerPrimitive } from "@/models/server";

// Funções para gerenciamento de servidores PostgreSQL
export async function createServer(server: IServerPrimitive): Promise<IServer> {
  return await invoke<IServer>("create_server", { ...server });
}

export async function getAllServers(): Promise<IServer[]> {
  return await invoke<IServer[]>("get_all_servers", {});
}

export async function getServerById(id: number): Promise<IServer> {
  return await invoke<IServer>("get_server_by_id", { id });
}

export async function updateServer(
  id: number,
  name: string,
  host: string,
  port: number,
  username: string,
  password: string,
  default_database?: string
): Promise<IServer> {
  return await invoke<IServer>("update_server", {
    id,
    name,
    host,
    port,
    username,
    password,
    default_database,
  });
}

export async function deleteServer(id: number): Promise<void> {
  return await invoke<void>("delete_server", { id });
}
