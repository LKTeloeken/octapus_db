import { describe, expect, it } from 'vitest';
import { SHORTCUT_IDS } from './actions';
import {
  eventToStroke,
  formatBinding,
  normalizeBinding,
  toCodeMirrorKey,
  type KeyEventLike,
} from './chord';
import {
  defaultBindings,
  findConflicts,
  matchShortcut,
  overridesFrom,
  resolveBindings,
  validateBinding,
} from './resolve';

const key = (key: string, patch: Partial<KeyEventLike> = {}): KeyEventLike => ({
  key,
  code: '',
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...patch,
});

describe('normalizeBinding', () => {
  it('põe as modificadoras na ordem e a letra em minúscula', () => {
    expect(normalizeBinding('shift+mod+J', true)).toBe('Mod+Shift+j');
    expect(normalizeBinding('Mod+k', false)).toBe('Mod+k');
  });

  it('Cmd é a Mod no macOS; Ctrl é a Mod fora dele', () => {
    expect(normalizeBinding('Cmd+k', true)).toBe('Mod+k');
    expect(normalizeBinding('Ctrl+k', true)).toBe('Ctrl+k');
    expect(normalizeBinding('Ctrl+k', false)).toBe('Mod+k');
    expect(normalizeBinding('Meta+k', false)).toBe('Meta+k');
  });

  it('letra maiúscula sozinha é Shift + letra', () => {
    expect(normalizeBinding('G', true)).toBe('Shift+g');
  });

  it('símbolo ignora o Shift (o caractere já diz)', () => {
    expect(normalizeBinding('Shift+/', true)).toBe('/');
  });

  it('nomes de tecla, apelidos e sequências', () => {
    expect(normalizeBinding('esc', true)).toBe('Escape');
    expect(normalizeBinding('mod+enter', true)).toBe('Mod+Enter');
    expect(normalizeBinding('f7', true)).toBe('F7');
    expect(normalizeBinding('g g', true)).toBe('g g');
    expect(normalizeBinding('Mod++', true)).toBe('Mod++');
  });

  it('o que não é tecla não vale', () => {
    expect(normalizeBinding('', true)).toBeNull();
    expect(normalizeBinding('Hyper+k', true)).toBeNull();
    expect(normalizeBinding('Mod+NotAKey', true)).toBeNull();
  });
});

describe('eventToStroke', () => {
  it('⌘ no macOS e Ctrl nos outros viram Mod', () => {
    expect(eventToStroke(key('k', { metaKey: true }), true)).toBe('Mod+k');
    expect(eventToStroke(key('k', { ctrlKey: true }), false)).toBe('Mod+k');
    expect(eventToStroke(key('k', { ctrlKey: true }), true)).toBe('Ctrl+k');
  });

  it('forma leniente: Ctrl vale como ⌘ só no macOS', () => {
    expect(eventToStroke(key('k', { ctrlKey: true }), true, true)).toBe(
      'Mod+k',
    );
    expect(eventToStroke(key('k', { metaKey: true }), true, true)).toBeNull();
    expect(eventToStroke(key('k', { ctrlKey: true }), false, true)).toBeNull();
  });

  it('Shift com letra, Caps Lock e o espaço', () => {
    expect(eventToStroke(key('G', { shiftKey: true }), true)).toBe('Shift+g');
    expect(eventToStroke(key('G'), true)).toBe('Shift+g');
    expect(eventToStroke(key(' '), true)).toBe('Space');
    expect(eventToStroke(key(' ', { shiftKey: true }), true)).toBe(
      'Shift+Space',
    );
  });

  it('o Option do macOS usa a tecla física', () => {
    expect(eventToStroke(key('∆', { altKey: true, code: 'KeyJ' }), true)).toBe(
      'Alt+j',
    );
  });

  it('só a modificadora não é tecla', () => {
    expect(eventToStroke(key('Shift', { shiftKey: true }), true)).toBeNull();
    expect(eventToStroke(key('Meta', { metaKey: true }), true)).toBeNull();
  });
});

describe('formatBinding e toCodeMirrorKey', () => {
  it('rótulos no macOS e fora dele', () => {
    expect(formatBinding('Mod+Shift+j', true)).toBe('⇧⌘J');
    expect(formatBinding('Mod+Shift+j', false)).toBe('Ctrl+Shift+J');
    expect(formatBinding('Mod+Enter', true)).toBe('⌘↵');
    expect(formatBinding('j', true)).toBe('j');
    expect(formatBinding('g g', true)).toBe('g g');
    expect(formatBinding('Mod++', true)).toBe('⌘+');
  });

  it('formato do CodeMirror', () => {
    expect(toCodeMirrorKey('Mod+Shift+Enter')).toBe('Mod-Shift-Enter');
    expect(toCodeMirrorKey('g g')).toBe('g g');
  });
});

describe('resolveBindings', () => {
  it('todos os padrões do registro são teclas válidas', () => {
    for (const mac of [true, false]) {
      const resolved = resolveBindings(undefined, mac);
      for (const id of SHORTCUT_IDS) {
        expect(resolved[id].length, id).toBeGreaterThan(0);
      }
    }
  });

  it('árvore e grade andam com as setas e com o hjkl do vim', () => {
    const resolved = resolveBindings(undefined, true);
    expect(resolved['tree.down']).toEqual(['ArrowDown', 'j']);
    expect(resolved['tree.up']).toEqual(['ArrowUp', 'k']);
    expect(resolved['tree.expand']).toEqual(['ArrowRight', 'l']);
    expect(resolved['tree.collapse']).toEqual(['ArrowLeft', 'h']);
    expect(resolved['tree.open']).toEqual(['Enter', 'o']);
    expect(resolved['grid.left']).toEqual(['ArrowLeft', 'h']);
  });

  it('padrão por plataforma', () => {
    expect(defaultBindings('editor.complete', true)).toEqual(['Shift+Space']);
    expect(defaultBindings('editor.complete', false)).toEqual(['Mod+Space']);
  });

  it('a troca substitui a lista; vazia tira o atalho; lixo é ignorado', () => {
    const resolved = resolveBindings(
      {
        'tree.down': ['j', 'ctrl+n', '???+x'],
        'app.commandPalette': [],
        'removed.action': ['Mod+x'],
      },
      true,
    );
    expect(resolved['tree.down']).toEqual(['j', 'Ctrl+n']);
    expect(resolved['app.commandPalette']).toEqual([]);
    expect(resolved).not.toHaveProperty('removed.action');
  });

  it('nas preferências só vai o que difere do padrão', () => {
    expect(
      overridesFrom(
        { 'tree.down': ['ArrowDown', 'j'], 'tree.up': ['k'] },
        { 'tree.down': ['n'], 'gone.action': ['x'] },
        true,
      ),
    ).toEqual({ 'tree.up': ['k'] });
  });
});

describe('findConflicts e validateBinding', () => {
  const resolved = resolveBindings(undefined, true);

  it('mesma tecla no mesmo contexto conflita', () => {
    expect(findConflicts('tree.up', 'j', resolved)).toEqual([
      { id: 'tree.down', binding: 'j', kind: 'same' },
    ]);
  });

  it('contextos que não se cruzam não conflitam', () => {
    // `j` na árvore e na grade: cada um com o seu foco
    expect(findConflicts('grid.down', 'j', resolved)).toEqual([]);
  });

  it('global x painel é sombra (o painel ganha com o foco nele)', () => {
    expect(findConflicts('tabs.close', 'Mod+w', resolved)).toEqual([
      { id: 'assistant.closeConversation', binding: 'Mod+w', kind: 'shadow' },
    ]);
  });

  it('uma tecla e uma sequência que começa com ela conflitam', () => {
    expect(findConflicts('tree.filter', 'g', resolved)).toEqual([
      { id: 'tree.first', binding: 'g g', kind: 'same' },
    ]);
  });

  it('atalho que vale digitando precisa de modificadora', () => {
    expect(validateBinding('app.commandPalette', 'p', true)).not.toBeNull();
    expect(validateBinding('app.commandPalette', 'Mod+p', true)).toBeNull();
    expect(validateBinding('table.toggleValuePanel', 'F8', true)).toBeNull();
    expect(validateBinding('editor.complete', 'Shift+Space', true)).toBeNull();
    expect(validateBinding('tree.down', 'n', true)).toBeNull();
    expect(validateBinding('palette.down', 'ArrowDown', true)).toBeNull();
  });

  it('atalhos do sistema são reservados', () => {
    expect(validateBinding('app.commandPalette', 'Mod+q', true)).not.toBeNull();
    expect(validateBinding('app.commandPalette', 'Mod+h', true)).not.toBeNull();
    expect(validateBinding('app.commandPalette', 'Mod+h', false)).toBeNull();
  });
});

describe('matchShortcut', () => {
  const resolved = resolveBindings(undefined, true);
  const treeIds = ['tree.down', 'tree.first', 'tree.last'] as const;

  it('acha a ação pela tecla, em qualquer das teclas dela', () => {
    expect(matchShortcut(key('j'), treeIds, resolved, null, true)).toEqual({
      kind: 'fire',
      id: 'tree.down',
    });
    expect(
      matchShortcut(key('ArrowDown'), treeIds, resolved, null, true),
    ).toEqual({ kind: 'fire', id: 'tree.down' });
  });

  it('gg: o primeiro g espera, o segundo dispara', () => {
    expect(matchShortcut(key('g'), treeIds, resolved, null, true)).toEqual({
      kind: 'pending',
      sequence: 'g',
    });
    expect(matchShortcut(key('g'), treeIds, resolved, 'g', true)).toEqual({
      kind: 'fire',
      id: 'tree.first',
    });
  });

  it('tecla que não continua a sequência vale sozinha', () => {
    expect(matchShortcut(key('j'), treeIds, resolved, 'g', true)).toEqual({
      kind: 'fire',
      id: 'tree.down',
    });
    expect(
      matchShortcut(key('G', { shiftKey: true }), treeIds, resolved, 'g', true),
    ).toEqual({ kind: 'fire', id: 'tree.last' });
  });

  it('acorde exige as modificadoras exatas', () => {
    expect(
      matchShortcut(key('j', { metaKey: true }), treeIds, resolved, null, true),
    ).toBeNull();
  });

  it('Ctrl vale como ⌘ no macOS, se nenhuma ação usar o Ctrl', () => {
    const ids = ['app.commandPalette'] as const;
    expect(
      matchShortcut(key('k', { ctrlKey: true }), ids, resolved, null, true),
    ).toEqual({ kind: 'fire', id: 'app.commandPalette' });

    const custom = resolveBindings({ 'tree.down': ['Ctrl+k'] }, true);
    expect(
      matchShortcut(
        key('k', { ctrlKey: true }),
        ['app.commandPalette', 'tree.down'],
        custom,
        null,
        true,
      ),
    ).toEqual({ kind: 'fire', id: 'tree.down' });
  });
});
