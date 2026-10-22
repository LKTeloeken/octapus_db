import { call } from './client';
import { RustCommand } from './commands';
import type {
  AdapterCapabilities,
  DbTypeCapabilities,
} from './types/capabilities.types';
import type { PoolStats, ServerInput } from './types/server.types';

/** database null → connects to the db type's default (postgres/admin/0) */
export function connect(
  serverId: number,
  database?: string | null,
): Promise<boolean> {
  return call<boolean>(RustCommand.Connect, {
    serverId,
    database: database ?? null,
  });
}

export function testConnection(serverId: number): Promise<boolean> {
  return call<boolean>(RustCommand.TestConnection, { serverId });
}

/**
 * Testa os dados do formulário sem gravar (conexão avulsa, ping, descarte).
 * Na edição, `serverId` deixa a senha em branco valer a salva. Falhar rejeita
 * com o erro do backend (`code` e a mensagem com o motivo).
 */
export function testServerInput(
  input: ServerInput,
  serverId?: number | null,
): Promise<void> {
  return call<void>(RustCommand.TestServerInput, {
    input,
    serverId: serverId ?? null,
  });
}

/** Without database → disconnects every database of the server */
export function disconnect(
  serverId: number,
  database?: string | null,
): Promise<void> {
  return call<void>(RustCommand.Disconnect, {
    serverId,
    database: database ?? null,
  });
}

/** Only Postgres returns stats */
export function getPoolStats(
  serverId: number,
  database: string,
): Promise<PoolStats | null> {
  return call<PoolStats | null>(RustCommand.GetPoolStats, {
    serverId,
    database,
  });
}

/** As capabilities de cada tipo de banco (estáticas — dependem só do tipo) */
export function getDbCapabilities(): Promise<DbTypeCapabilities[]> {
  return call<DbTypeCapabilities[]>(RustCommand.GetDbCapabilities);
}

export function getCapabilities(
  serverId: number,
): Promise<AdapterCapabilities> {
  return call<AdapterCapabilities>(RustCommand.GetCapabilities, { serverId });
}
