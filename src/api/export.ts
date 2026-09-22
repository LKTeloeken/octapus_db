import { call } from './client';
import { RustCommand } from './commands';

/**
 * Grava o arquivo já serializado no caminho escolhido pelo usuário no diálogo
 * de salvar. O backend só grava — a serialização (CSV/JSON/SQL) é do front.
 */
export function writeExportFile(path: string, contents: string): Promise<void> {
  return call<void>(RustCommand.WriteExportFile, { path, contents });
}
