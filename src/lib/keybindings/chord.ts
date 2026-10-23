import { isMac } from '@/lib/platform';

/**
 * O formato dos atalhos: cada tecla é um acorde (`"Mod+Shift+J"`) e uma
 * sequência separa os acordes por espaço (`"g g"`, o `gg` do vim).
 *
 * `Mod` é a modificadora da plataforma — ⌘ no macOS, Ctrl nos demais. No
 * macOS o `Ctrl` é outra tecla (⌃); fora dele `Ctrl` vira `Mod` e `Meta` é a
 * tecla do Windows/Super. Tudo é normalizado para uma forma canônica, então
 * casar um evento com um atalho é comparar duas strings.
 */

/** O que o casamento precisa do evento (o `KeyboardEvent` do React serve) */
export type KeyEventLike = Pick<
  KeyboardEvent,
  'key' | 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'
>;

interface Stroke {
  mod: boolean;
  ctrl: boolean;
  meta: boolean;
  alt: boolean;
  shift: boolean;
  key: string;
}

/** Teclas com nome, na grafia canônica (a do `KeyboardEvent.key`) */
const NAMED_KEYS = [
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Enter',
  'Escape',
  'Tab',
  'Space',
  'Backspace',
  'Delete',
  'Insert',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  ...Array.from({ length: 12 }, (_, i) => `F${i + 1}`),
];
const NAMED_BY_LOWER = new Map(
  NAMED_KEYS.map(name => [name.toLowerCase(), name]),
);
const KEY_ALIASES: Record<string, string> = {
  esc: 'Escape',
  return: 'Enter',
  del: 'Delete',
  up: 'ArrowUp',
  down: 'ArrowDown',
  left: 'ArrowLeft',
  right: 'ArrowRight',
  ' ': 'Space',
  spacebar: 'Space',
};

/** Só a modificadora, sem tecla: não é atalho (ainda) */
const MODIFIER_KEYS = new Set([
  'Shift',
  'Control',
  'Meta',
  'Alt',
  'AltGraph',
  'CapsLock',
  'OS',
  'Fn',
  'Hyper',
  'Super',
]);

const isLetter = (char: string) => char.toLowerCase() !== char.toUpperCase();

/**
 * Um caractere imprimível que não é letra (`/`, `1`, `?`): o próprio caractere
 * já diz se o Shift estava apertado — e depende do layout do teclado (o `/`
 * do ABNT2 não usa Shift; o do alemão, sim). Por isso o Shift não conta.
 */
const isSymbol = (key: string) => key.length === 1 && !isLetter(key);

function normalizeKey(raw: string): string | null {
  if (raw === '') return null;
  const alias = KEY_ALIASES[raw.toLowerCase()] ?? KEY_ALIASES[raw];
  if (alias) return alias;
  if (raw.length === 1) return isLetter(raw) ? raw.toLowerCase() : raw;
  return NAMED_BY_LOWER.get(raw.toLowerCase()) ?? null;
}

function strokeToString({ mod, ctrl, meta, alt, shift, key }: Stroke): string {
  const parts: string[] = [];
  if (mod) parts.push('Mod');
  if (ctrl) parts.push('Ctrl');
  if (meta) parts.push('Meta');
  if (alt) parts.push('Alt');
  if (shift && !isSymbol(key)) parts.push('Shift');
  parts.push(key);
  return parts.join('+');
}

function parseStroke(text: string, mac: boolean): Stroke | null {
  // `"Mod++"`: a tecla é o próprio `+`
  const plusKey = text.endsWith('++') || text === '+';
  const body = plusKey ? text.slice(0, -1) : text;
  const parts = body.split('+').filter(part => part !== '');
  const rawKey = plusKey ? '+' : parts.pop();
  if (!rawKey) return null;

  const stroke: Stroke = {
    mod: false,
    ctrl: false,
    meta: false,
    alt: false,
    shift: false,
    key: '',
  };
  for (const part of parts) {
    switch (part.toLowerCase()) {
      case 'mod':
        stroke.mod = true;
        break;
      case 'cmd':
      case 'command':
      case 'meta':
      case 'super':
      case 'win':
        if (mac) stroke.mod = true;
        else stroke.meta = true;
        break;
      case 'ctrl':
      case 'control':
        if (mac) stroke.ctrl = true;
        else stroke.mod = true;
        break;
      case 'alt':
      case 'option':
      case 'opt':
        stroke.alt = true;
        break;
      case 'shift':
        stroke.shift = true;
        break;
      default:
        return null;
    }
  }

  // Letra maiúscula sozinha (`"G"`) é o mesmo que `"Shift+g"`
  if (
    rawKey.length === 1 &&
    isLetter(rawKey) &&
    rawKey !== rawKey.toLowerCase()
  ) {
    stroke.shift = true;
  }
  const key = normalizeKey(rawKey);
  if (!key) return null;
  stroke.key = key;
  return stroke;
}

/**
 * A forma canônica de um atalho (`"cmd+shift+j"` → `"Mod+Shift+j"`), ou
 * `null` se não for válido.
 */
export function normalizeBinding(
  binding: string,
  mac: boolean = isMac,
): string | null {
  const strokes = binding.trim().split(/\s+/);
  if (strokes.length === 0 || strokes[0] === '') return null;
  const parsed: string[] = [];
  for (const text of strokes) {
    const stroke = parseStroke(text, mac);
    if (!stroke) return null;
    parsed.push(strokeToString(stroke));
  }
  return parsed.join(' ');
}

/** As teclas de um atalho canônico (`"g g"` → `["g", "g"]`) */
export const bindingStrokes = (binding: string) => binding.split(' ');

function eventKey(event: KeyEventLike): string | null {
  if (MODIFIER_KEYS.has(event.key)) return null;
  // O Option do macOS troca a letra por outro caractere (⌥J → ∆): a tecla
  // física é que vale.
  if (event.altKey && event.code) {
    const letter = /^Key([A-Z])$/.exec(event.code);
    if (letter) return letter[1].toLowerCase();
    const digit = /^Digit(\d)$/.exec(event.code);
    if (digit) return digit[1];
  }
  return normalizeKey(event.key);
}

/**
 * A tecla do evento na forma canônica — `null` para só uma modificadora.
 * `lenient`: no macOS, o Ctrl também vale como `Mod` (o app sempre aceitou
 * Ctrl+K além de ⌘K); o dispatcher tenta essa forma só se a exata não casar.
 */
export function eventToStroke(
  event: KeyEventLike,
  mac: boolean = isMac,
  lenient = false,
): string | null {
  const key = eventKey(event);
  if (!key) return null;
  const letterShift =
    event.key.length === 1 &&
    isLetter(event.key) &&
    event.key !== event.key.toLowerCase();

  const stroke: Stroke = {
    mod: mac ? event.metaKey : event.ctrlKey,
    ctrl: mac && event.ctrlKey,
    meta: !mac && event.metaKey,
    alt: event.altKey,
    // Caps Lock (`"G"` sem Shift) conta como Shift, como no vim
    shift: event.shiftKey || letterShift,
    key,
  };
  if (lenient) {
    if (!mac || !stroke.ctrl || stroke.mod) return null;
    stroke.ctrl = false;
    stroke.mod = true;
  }
  return strokeToString(stroke);
}

/** O acorde tem alguma modificadora (fora o Shift)? */
export const hasModifier = (stroke: string) =>
  /(^|\+)(Mod|Ctrl|Meta|Alt)\+/.test(stroke);

/** Tecla F (`F7`): vale como atalho global mesmo sem modificadora */
export const isFunctionKey = (stroke: string) => /(^|\+)F\d{1,2}$/.test(stroke);

// ── Rótulos ─────────────────────────────────────────────────────────────────

const MAC_SYMBOLS: Record<string, string> = {
  Mod: '⌘',
  Ctrl: '⌃',
  Alt: '⌥',
  Shift: '⇧',
};
const OTHER_NAMES: Record<string, string> = {
  Mod: 'Ctrl',
  Ctrl: 'Ctrl',
  Meta: 'Win',
  Alt: 'Alt',
  Shift: 'Shift',
};
const KEY_LABELS: Record<string, string> = {
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Enter: '↵',
  Escape: 'esc',
  Tab: 'tab',
  Space: 'space',
  Backspace: '⌫',
  Delete: 'del',
  PageUp: 'PgUp',
  PageDown: 'PgDn',
};

function formatStroke(stroke: string, mac: boolean): string {
  const parts = stroke === '+' ? ['+'] : stroke.split(/\+(?!$)/);
  const key = parts.pop() ?? '';
  const modifiers = parts;
  const keyLabel =
    KEY_LABELS[key] ??
    // Letra com modificadora em maiúscula (⌘K); sozinha, como se digita (j)
    (modifiers.length > 0 ? key.toUpperCase() : key);

  if (mac) {
    // A ordem do macOS: ⌃⌥⇧⌘
    const order = ['Ctrl', 'Alt', 'Shift', 'Mod'];
    const symbols = order
      .filter(name => modifiers.includes(name))
      .map(name => MAC_SYMBOLS[name])
      .join('');
    return `${symbols}${keyLabel}`;
  }
  return [...modifiers.map(name => OTHER_NAMES[name] ?? name), keyLabel].join(
    '+',
  );
}

/** Rótulo para `Kbd` e tooltips: `"Mod+Shift+j"` → `⇧⌘J` / `Ctrl+Shift+J` */
export function formatBinding(binding: string, mac: boolean = isMac): string {
  return bindingStrokes(binding)
    .map(stroke => formatStroke(stroke, mac))
    .join(' ');
}

/** O atalho no formato do keymap do CodeMirror (`"Mod+Enter"` → `"Mod-Enter"`) */
export function toCodeMirrorKey(binding: string): string {
  return bindingStrokes(binding)
    .map(stroke => {
      const parts = stroke === '+' ? ['+'] : stroke.split(/\+(?!$)/);
      return parts.join('-');
    })
    .join(' ');
}
