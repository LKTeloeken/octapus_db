import { call } from './client';
import { RustCommand } from './commands';

/** Abre a pasta do log do app no gerenciador de arquivos */
export function openLogDir(): Promise<void> {
  return call<void>(RustCommand.OpenLogDir);
}

/**
 * Registra no log do app o estado da webview: a rede mudou (`hiddenMinutes`
 * ausente) ou a janela voltou depois de `hiddenMinutes` escondida.
 */
export function reportWebviewState(
  online: boolean,
  hiddenMinutes?: number,
): Promise<void> {
  return call<void>(RustCommand.ReportWebviewState, {
    online,
    hiddenMinutes: hiddenMinutes ?? null,
  });
}
