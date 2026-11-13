import { call } from './client';
import { RustCommand } from './commands';
import type { CliStatus } from './types/cli.types';

/** O `octapus` de terminal nesta máquina: instalado? onde? está no PATH? */
export function getCliStatus(): Promise<CliStatus> {
  return call<CliStatus>(RustCommand.CliStatus);
}

/**
 * Baixa da release desta versão do app o `octapus` do sistema, confere o
 * SHA-256 e instala na pasta de binários do usuário.
 */
export function installCli(): Promise<CliStatus> {
  return call<CliStatus>(RustCommand.CliInstall);
}
