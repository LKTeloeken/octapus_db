import { toBackendError } from './errors';
import { handlers } from './handlers';
import { useMockStore } from './mock-store';

/** Plugins do Tauri não passam pela injeção de latência/erro do painel */
const isPluginCommand = (command: string) => command.startsWith('plugin:');

/** ±20% de variação, para o loading não parecer cronometrado */
function jitter(ms: number) {
  if (ms <= 0) return 0;
  return Math.round(ms * (0.8 + Math.random() * 0.4));
}

const sleep = (ms: number) =>
  ms > 0 ? new Promise(resolve => setTimeout(resolve, ms)) : Promise.resolve();

/**
 * Ponto único de entrada do backend simulado. Os comandos do app rejeitam com
 * `{ code, message }`, igual ao `Error` serializado dos comandos Rust; comando
 * sem handler e plugins rejeitam com string, como no Tauri. É o que o `call()`
 * em src/api/client.ts espera para montar o `ApiError`.
 */
export async function route(
  command: string,
  args: Record<string, unknown> = {},
): Promise<unknown> {
  const handler = handlers[command];
  if (!handler) {
    throw `Unsupported command: "${command}" não tem handler em src/mocks/handlers.ts`;
  }

  if (isPluginCommand(command)) return handler(args);

  const { latencyMs, consumeFailure } = useMockStore.getState();
  await sleep(jitter(latencyMs));

  try {
    const failure = consumeFailure();
    if (failure) throw failure;

    return await handler(args);
  } catch (thrown) {
    throw toBackendError(thrown);
  }
}
