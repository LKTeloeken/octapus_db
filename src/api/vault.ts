import { call } from './client';
import { RustCommand } from './commands';
import type { VaultStatus } from './types/vault.types';

export function getVaultStatus(): Promise<VaultStatus> {
  return call<VaultStatus>(RustCommand.VaultStatus);
}

/** Senha errada rejeita — o GCM autentica o embrulho da chave */
export function unlockVault(password: string): Promise<void> {
  return call<void>(RustCommand.VaultUnlock, { password });
}

/** Tranca e derruba as conexões abertas */
export function lockVault(): Promise<void> {
  return call<void>(RustCommand.VaultLock);
}

/** Instantâneo: nenhum dado é re-criptografado, só o vault.key muda de formato */
export function enableMasterPassword(password: string): Promise<void> {
  return call<void>(RustCommand.VaultEnableMasterPassword, { password });
}

export function disableMasterPassword(password: string): Promise<void> {
  return call<void>(RustCommand.VaultDisableMasterPassword, { password });
}

/** **Destrutivo:** descarta a chave e apaga toda senha e URI guardadas */
export function resetVault(): Promise<void> {
  return call<void>(RustCommand.VaultReset);
}
