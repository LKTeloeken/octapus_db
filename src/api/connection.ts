import { call } from './client';
import { RustCommand } from './commands';
import type { AdapterCapabilities } from './types/capabilities.types';
import type { PoolStats } from './types/server.types';

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

export function getCapabilities(
  serverId: number,
): Promise<AdapterCapabilities> {
  return call<AdapterCapabilities>(RustCommand.GetCapabilities, { serverId });
}
