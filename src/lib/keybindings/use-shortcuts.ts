import type { KeyBinding } from '@codemirror/view';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import { isMac } from '@/lib/platform';
import { useKeybindingsStore } from '@/stores/keybindings-store';
import { shortcutAction, type ShortcutId } from './actions';
import { eventToStroke, formatBinding, toCodeMirrorKey } from './chord';
import { matchShortcut } from './resolve';

/** A tecla seguinte de uma sequência (`g g`) vale até este tempo depois */
export const SEQUENCE_WINDOW_MS = 600;

type AnyKeyboardEvent = KeyboardEvent | ReactKeyboardEvent;

/**
 * Faz a ação. Devolver `false` diz "não era comigo" (sem aba 3 para ir, foco
 * fora do painel): o evento segue sem `preventDefault` para os outros.
 */
export type ShortcutHandler = (event: AnyKeyboardEvent) => void | boolean;

export type ShortcutHandlers = Partial<Record<ShortcutId, ShortcutHandler>>;

export interface ShortcutOptions {
  /**
   * `window` (padrão): o hook registra o listener. `element`: só devolve o
   * `onKeyDown` para o contêiner focável (árvore, grade, paleta).
   */
  target?: 'window' | 'element';
  /** Fase de captura do `window` — passa na frente dos listeners comuns */
  capture?: boolean;
  enabled?: boolean;
  /** Condição de contexto além do foco num campo (diálogo aberto, escopo…) */
  when?: (event: AnyKeyboardEvent) => boolean;
  /** Ignora a tecla segurada (auto-repeat) */
  ignoreRepeat?: boolean;
}

/** O foco está num campo de texto (input, textarea, CodeMirror)? */
export const isTypingTarget = (target: EventTarget | null) => {
  const element = target as HTMLElement | null;
  return (
    !!element &&
    (element.tagName === 'INPUT' ||
      element.tagName === 'TEXTAREA' ||
      element.tagName === 'SELECT' ||
      element.isContentEditable === true)
  );
};

/** Estado de um `useShortcuts` entre um evento e outro */
export interface ShortcutRuntime {
  /** Teclas já apertadas de uma sequência, e quando veio a última */
  pending: { sequence: string; at: number } | null;
}

/**
 * Decide e executa o atalho do evento — o miolo do `useShortcuts`, fora do
 * React para poder ser testado com eventos de mentira.
 */
export function handleShortcutEvent(
  event: AnyKeyboardEvent,
  handlers: ShortcutHandlers,
  options: ShortcutOptions,
  runtime: ShortcutRuntime,
  mac: boolean = isMac,
): void {
  const { enabled = true, when, ignoreRepeat } = options;
  if (!enabled || event.defaultPrevented) return;
  if (ignoreRepeat && event.repeat) return;

  const { resolved, recording } = useKeybindingsStore.getState();
  if (recording) return;

  // Só a modificadora (o Shift antes do G): não interrompe a sequência
  if (eventToStroke(event, mac) === null) return;

  const typing = isTypingTarget(event.target);
  const ids = (Object.keys(handlers) as ShortcutId[]).filter(
    id => handlers[id] && (!typing || shortcutAction(id).allowWhileTyping),
  );
  if (ids.length === 0) return;
  if (when && !when(event)) return;

  const now = event.timeStamp;
  const previous =
    runtime.pending && now - runtime.pending.at <= SEQUENCE_WINDOW_MS
      ? runtime.pending.sequence
      : null;
  const match = matchShortcut(event, ids, resolved, previous, mac);
  runtime.pending = null;
  if (!match) return;

  if (match.kind === 'pending') {
    event.preventDefault();
    runtime.pending = { sequence: match.sequence, at: now };
    return;
  }
  if (handlers[match.id]?.(event) !== false) event.preventDefault();
}

/**
 * O hook de atalhos do app: liga as ações do registro (`actions.ts`) aos
 * handlers, com as teclas que o usuário configurou.
 *
 * Cuida do que todo atalho precisa: tecla em campo de texto (só as ações com
 * `allowWhileTyping`), evento já tratado por outro (`defaultPrevented` — o
 * assistente, em captura, vence as abas), sequências como o `gg` e o
 * `preventDefault`. As regras de contexto que só a tela conhece entram no
 * `when`.
 *
 * ```ts
 * useShortcuts({ 'app.commandPalette': togglePalette });
 * const { onKeyDown } = useShortcuts(
 *   { 'tree.down': () => move(1) },
 *   { target: 'element' },
 * );
 * ```
 */
export function useShortcuts(
  handlers: ShortcutHandlers,
  options: ShortcutOptions = {},
) {
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const runtime = useRef<ShortcutRuntime>({ pending: null });

  const onKeyDown = useCallback(
    (event: AnyKeyboardEvent) =>
      handleShortcutEvent(
        event,
        handlersRef.current,
        optionsRef.current,
        runtime.current,
      ),
    [],
  );

  const { target = 'window', capture = false } = options;
  useEffect(() => {
    if (target !== 'window') return;
    window.addEventListener('keydown', onKeyDown, capture);
    return () => window.removeEventListener('keydown', onKeyDown, capture);
  }, [target, capture, onKeyDown]);

  return { onKeyDown };
}

/** As teclas da ação, já como rótulo (`⌘K`, `Ctrl+K`) */
export function useShortcutLabels(id: ShortcutId): string[] {
  const bindings = useKeybindingsStore(state => state.resolved[id]);
  return useMemo(
    () => bindings.map(binding => formatBinding(binding)),
    [bindings],
  );
}

/**
 * O rótulo da primeira tecla da ação, para `Kbd` e tooltips — `null` se o
 * usuário a deixou sem atalho (aí a dica some).
 */
export function useShortcutLabel(id: ShortcutId): string | null {
  return useShortcutLabels(id)[0] ?? null;
}

/** Texto de tooltip com o atalho, se houver: `Recarregar (⌘R)` */
export const withShortcut = (text: string, label: string | null) =>
  label ? `${text} (${label})` : text;

/**
 * Os atalhos do registro no formato do keymap do CodeMirror, para os editores
 * (que tratam as teclas dentro deles mesmos). Muda quando o usuário troca a
 * tecla — quem usa reconfigura a extensão.
 */
export function useCodeMirrorShortcuts(
  handlers: Partial<Record<ShortcutId, KeyBinding['run']>>,
  extra: Omit<KeyBinding, 'key' | 'run'> = {},
): KeyBinding[] {
  const resolved = useKeybindingsStore(state => state.resolved);
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;
  const ids = Object.keys(handlers).join(',');

  return useMemo(
    () =>
      (ids.split(',').filter(Boolean) as ShortcutId[]).flatMap(id =>
        resolved[id].map(binding => ({
          ...extra,
          key: toCodeMirrorKey(binding),
          run: view => handlersRef.current[id]?.(view) ?? false,
        })),
      ),
    // `extra` é configuração fixa de quem chama
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [ids, resolved],
  );
}
