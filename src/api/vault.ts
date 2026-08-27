import { call } from './client';
import { RustCommand } from './commands';
import type { VaultStatus } from './types/vault.types';

/** Lido do canário, medido uma vez no boot do backend */
export function getVaultStatus(): Promise<VaultStatus> {
  return call<VaultStatus>(RustCommand.VaultStatus);
}
