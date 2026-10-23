import { afterEach, describe, expect, it, vi } from 'vitest';
import { useKeybindingsStore } from '@/stores/keybindings-store';
import {
  handleShortcutEvent,
  SEQUENCE_WINDOW_MS,
  type ShortcutHandlers,
  type ShortcutOptions,
  type ShortcutRuntime,
} from './use-shortcuts';

interface FakeEvent {
  key: string;
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  repeat: boolean;
  timeStamp: number;
  target: EventTarget | null;
  defaultPrevented: boolean;
  preventDefault: () => void;
}

/** Um keydown de mentira (macOS: ⌘ = `metaKey`) */
const keydown = (key: string, patch: Partial<FakeEvent> = {}): FakeEvent => {
  const event: FakeEvent = {
    key,
    code: '',
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    repeat: false,
    timeStamp: 0,
    target: {
      tagName: 'DIV',
      isContentEditable: false,
    } as unknown as EventTarget,
    defaultPrevented: false,
    preventDefault: () => {
      event.defaultPrevented = true;
    },
    ...patch,
  };
  return event;
};

const input = {
  tagName: 'INPUT',
  isContentEditable: false,
} as unknown as EventTarget;

/** Um `useShortcuts` montado: handlers, opções e o estado entre eventos */
const listener = (
  handlers: ShortcutHandlers,
  options: ShortcutOptions = {},
) => {
  const runtime: ShortcutRuntime = { pending: null };
  return (event: FakeEvent) =>
    handleShortcutEvent(
      event as unknown as KeyboardEvent,
      handlers,
      options,
      runtime,
      true,
    );
};

afterEach(() => {
  useKeybindingsStore.getState().setOverrides(undefined);
  useKeybindingsStore.getState().setRecording(false);
});

describe('handleShortcutEvent', () => {
  it('dispara a ação e trata o evento', () => {
    const toggle = vi.fn();
    const event = keydown('k', { metaKey: true });
    listener({ 'app.commandPalette': toggle })(event);
    expect(toggle).toHaveBeenCalledOnce();
    expect(event.defaultPrevented).toBe(true);
  });

  it('segue a tecla configurada pelo usuário', () => {
    useKeybindingsStore.getState().setOverrides({
      'app.commandPalette': ['Mod+p'],
    });
    const toggle = vi.fn();
    const on = listener({ 'app.commandPalette': toggle });
    on(keydown('k', { metaKey: true }));
    expect(toggle).not.toHaveBeenCalled();
    on(keydown('p', { metaKey: true }));
    expect(toggle).toHaveBeenCalledOnce();
  });

  it('gravando um atalho, nenhum dispara', () => {
    useKeybindingsStore.getState().setRecording(true);
    const toggle = vi.fn();
    listener({ 'app.commandPalette': toggle })(keydown('k', { metaKey: true }));
    expect(toggle).not.toHaveBeenCalled();
  });

  it('num campo de texto só valem as ações com allowWhileTyping', () => {
    const down = vi.fn();
    const palette = vi.fn();
    const on = listener({ 'tree.down': down, 'app.commandPalette': palette });
    on(keydown('j', { target: input }));
    on(keydown('k', { metaKey: true, target: input }));
    expect(down).not.toHaveBeenCalled();
    expect(palette).toHaveBeenCalledOnce();
  });

  it('a condição `when` da tela pode recusar', () => {
    const toggle = vi.fn();
    listener(
      { 'app.commandPalette': toggle },
      { when: () => false },
    )(keydown('k', { metaKey: true }));
    expect(toggle).not.toHaveBeenCalled();
  });

  it('gg dentro da janela de tempo; fora dela, recomeça', () => {
    const first = vi.fn();
    const on = listener({ 'tree.first': first });
    const g1 = keydown('g', { timeStamp: 0 });
    on(g1);
    expect(g1.defaultPrevented).toBe(true);
    on(keydown('g', { timeStamp: 100 }));
    expect(first).toHaveBeenCalledOnce();

    on(keydown('g', { timeStamp: 1000 }));
    on(keydown('g', { timeStamp: 1000 + SEQUENCE_WINDOW_MS + 1 }));
    expect(first).toHaveBeenCalledOnce();
  });

  it('o Shift do G não interrompe a sequência pendente', () => {
    const last = vi.fn();
    const on = listener({ 'tree.first': vi.fn(), 'tree.last': last });
    on(keydown('g'));
    on(keydown('Shift', { shiftKey: true }));
    on(keydown('G', { shiftKey: true }));
    expect(last).toHaveBeenCalledOnce();
  });
});

describe('Cmd/Ctrl+W: assistente x abas de query', () => {
  /** O assistente em captura, as abas em bolha — a ordem do navegador */
  const setup = (focusInPanel: boolean) => {
    const closeConversation = vi.fn();
    const closeTab = vi.fn();
    const assistant = listener(
      {
        'assistant.closeConversation': () => {
          if (!focusInPanel) return false;
          closeConversation();
        },
      },
      { capture: true },
    );
    const tabs = listener({ 'tabs.close': closeTab });
    const press = (event: FakeEvent) => {
      assistant(event);
      tabs(event);
    };
    return { press, closeConversation, closeTab };
  };

  it('com o foco no painel, fecha a conversa e não a aba', () => {
    const { press, closeConversation, closeTab } = setup(true);
    press(keydown('w', { metaKey: true }));
    expect(closeConversation).toHaveBeenCalledOnce();
    expect(closeTab).not.toHaveBeenCalled();
  });

  it('fora do painel, o assistente passa e a aba fecha', () => {
    const { press, closeConversation, closeTab } = setup(false);
    const event = keydown('w', { metaKey: true });
    press(event);
    expect(closeConversation).not.toHaveBeenCalled();
    expect(closeTab).toHaveBeenCalledOnce();
    expect(event.defaultPrevented).toBe(true);
  });

  it('ir para uma aba que não existe deixa o evento passar', () => {
    const event = keydown('3', { metaKey: true });
    listener({ 'tabs.goto3': () => false })(event);
    expect(event.defaultPrevented).toBe(false);
  });
});
