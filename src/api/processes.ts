import { call } from './client';
import { RustCommand } from './commands';
import type { DatabaseProcess } from './types/processes.types';

export function listProcesses(serverId: number): Promise<DatabaseProcess[]> {
  return call<DatabaseProcess[]>(RustCommand.ListProcesses, { serverId });
}

export function terminateProcess(
  serverId: number,
  pid: number,
): Promise<boolean> {
  return call<boolean>(RustCommand.TerminateProcess, { serverId, pid });
}
