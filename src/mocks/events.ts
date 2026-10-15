/**
 * Eventos globais do Tauri no mock. O `listen` do `@tauri-apps/api/event`
 * registra um callback (via `transformCallback`, que o `mockIPC` guarda em
 * `window._<id>`) e chama `plugin:event|listen`; aqui guardamos os ids por nome
 * de evento para o mock poder emitir — o `@tauri-apps/api` 2.5 não traz isso
 * pronto.
 */

type CallbackWindow = Window &
  Record<string, ((event: unknown) => void) | undefined>;

const listeners = new Map<string, Map<number, number>>();
let nextEventId = 0;

export function registerListener(event: string, handler: number): number {
  const id = ++nextEventId;
  let byId = listeners.get(event);
  if (!byId) {
    byId = new Map();
    listeners.set(event, byId);
  }
  byId.set(id, handler);
  return id;
}

export function unregisterListener(event: string, eventId: number): void {
  listeners.get(event)?.delete(eventId);
}

/** O `@tauri-apps/api` 2.11 guarda os callbacks num mapa e roda por id */
type TauriInternals = {
  runCallback?: (id: number, data: unknown) => void;
};

export function mockEmit(event: string, payload: unknown): void {
  const target = window as unknown as CallbackWindow;
  const internals = (window as unknown as { __TAURI_INTERNALS__?: TauriInternals })
    .__TAURI_INTERNALS__;
  listeners.get(event)?.forEach((handler, id) => {
    const message = { event, id, payload };
    // Antes do 2.11 o callback ficava em `window._<id>`
    if (internals?.runCallback) internals.runCallback(handler, message);
    else target[`_${handler}`]?.(message);
  });
}
