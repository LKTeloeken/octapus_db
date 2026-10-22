import { describe, expect, it } from 'vitest';
import { queryTabShortcut } from '@/features/query-tabs/use-query-tabs';
import { assistantShortcut, type ShortcutKey } from './assistant-shortcuts';

const key = (key: string, patch: Partial<ShortcutKey> = {}): ShortcutKey => ({
  key,
  metaKey: true,
  ctrlKey: false,
  shiftKey: false,
  altKey: false,
  ...patch,
});

describe('atalhos do assistente', () => {
  it('Cmd/Ctrl+J abre e fecha o painel de qualquer lugar', () => {
    expect(assistantShortcut(key('j'), false)).toBe('toggle-panel');
    expect(
      assistantShortcut(key('J', { metaKey: false, ctrlKey: true }), true),
    ).toBe('toggle-panel');
  });

  it('com foco no painel, W e T são das conversas', () => {
    expect(assistantShortcut(key('w'), true)).toBe('close-tab');
    expect(assistantShortcut(key('t'), true)).toBe('new-tab');
  });

  it('fora do painel, W e T não são do assistente', () => {
    expect(assistantShortcut(key('w'), false)).toBeNull();
    expect(assistantShortcut(key('t'), false)).toBeNull();
  });

  it('Cmd/Ctrl+Shift+J entra e sai do modo agente de qualquer lugar', () => {
    expect(assistantShortcut(key('J', { shiftKey: true }), false)).toBe(
      'toggle-agent-mode',
    );
    expect(
      assistantShortcut(
        key('j', { metaKey: false, ctrlKey: true, shiftKey: true }),
        true,
      ),
    ).toBe('toggle-agent-mode');
  });

  it('sem modificadora, ou com Shift/Alt, nada', () => {
    expect(assistantShortcut(key('w', { metaKey: false }), true)).toBeNull();
    expect(assistantShortcut(key('w', { shiftKey: true }), true)).toBeNull();
    expect(assistantShortcut(key('t', { shiftKey: true }), true)).toBeNull();
    expect(assistantShortcut(key('j', { altKey: true }), true)).toBeNull();
  });
});

describe('Cmd/Ctrl+W das abas de query', () => {
  const event = (defaultPrevented: boolean) => ({
    key: 'w',
    metaKey: true,
    ctrlKey: false,
    defaultPrevented,
  });

  it('fecha a aba de query quando ninguém tratou o evento', () => {
    expect(queryTabShortcut(event(false))).toEqual({ kind: 'close' });
  });

  it('não fecha quando o assistente já tratou (foco nas conversas)', () => {
    expect(queryTabShortcut(event(true))).toBeNull();
  });

  it('Cmd/Ctrl+1..9 continua indo à aba', () => {
    expect(queryTabShortcut({ ...event(false), key: '3' })).toEqual({
      kind: 'goto',
      index: 2,
    });
  });
});
