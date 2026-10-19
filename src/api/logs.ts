import { call } from './client';
import { RustCommand } from './commands';

/** Abre a pasta do log do app no gerenciador de arquivos */
export function openLogDir(): Promise<void> {
  return call<void>(RustCommand.OpenLogDir);
}
