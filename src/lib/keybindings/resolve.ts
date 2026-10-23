import { isMac } from '@/lib/platform';
import {
  contextsOverlap,
  isShortcutId,
  SHORTCUT_ACTIONS,
  SHORTCUT_IDS,
  shortcutAction,
  type ShortcutId,
} from './actions';
import {
  bindingStrokes,
  eventToStroke,
  hasModifier,
  isFunctionKey,
  normalizeBinding,
  type KeyEventLike,
} from './chord';

/** Teclas de cada ação, já canônicas */
export type ResolvedBindings = Record<ShortcutId, string[]>;

/** O que foi trocado pelo usuário (o `keybindings` das preferências) */
export type ShortcutOverrides = Partial<Record<string, string[]>>;

const canonical = (bindings: string[], mac: boolean) => {
  const seen = new Set<string>();
  for (const binding of bindings) {
    const normalized = normalizeBinding(binding, mac);
    if (normalized) seen.add(normalized);
  }
  return Array.from(seen);
};

/** O padrão da ação nesta plataforma, canônico */
export function defaultBindings(
  id: ShortcutId,
  mac: boolean = isMac,
): string[] {
  const { defaults } = shortcutAction(id);
  return canonical(
    Array.isArray(defaults) ? defaults : mac ? defaults.mac : defaults.other,
    mac,
  );
}

/**
 * Padrões + o que o usuário trocou. A troca substitui a lista inteira da ação
 * (lista vazia = sem atalho); ids que não existem mais e teclas ilegíveis são
 * ignorados.
 */
export function resolveBindings(
  overrides: ShortcutOverrides | undefined,
  mac: boolean = isMac,
): ResolvedBindings {
  const resolved = {} as ResolvedBindings;
  for (const id of SHORTCUT_IDS) {
    const custom = overrides?.[id];
    resolved[id] = custom ? canonical(custom, mac) : defaultBindings(id, mac);
  }
  return resolved;
}

/**
 * O que gravar nas preferências: só as ações diferentes do padrão. Voltar
 * uma ação ao padrão tira a chave.
 */
export function overridesFrom(
  resolved: Partial<ResolvedBindings>,
  current: ShortcutOverrides | undefined,
  mac: boolean = isMac,
): Record<string, string[]> {
  const next: Record<string, string[]> = {};
  for (const [id, bindings] of Object.entries({ ...current, ...resolved })) {
    if (!isShortcutId(id) || !bindings) continue;
    const defaults = defaultBindings(id, mac);
    const same =
      bindings.length === defaults.length &&
      bindings.every((binding, i) => binding === defaults[i]);
    if (!same) next[id] = bindings;
  }
  return next;
}

// ── Conflitos ───────────────────────────────────────────────────────────────

export interface ShortcutConflict {
  id: ShortcutId;
  binding: string;
  /**
   * `same`: as duas valem no mesmo lugar — só uma dispararia.
   * `shadow`: uma é global e a outra de um painel; com o foco no painel, a
   * dele ganha (como o Cmd+W do assistente sobre o das abas).
   */
  kind: 'same' | 'shadow';
}

/** Uma sequência e o começo dela (`g` e `g g`) também brigam pela tecla */
const bindingsClash = (a: string, b: string) => {
  if (a === b) return true;
  const sa = bindingStrokes(a);
  const sb = bindingStrokes(b);
  const [short, long] = sa.length < sb.length ? [sa, sb] : [sb, sa];
  return short.every((stroke, i) => long[i] === stroke);
};

/** As outras ações que usam `binding` onde a ação `id` também vale */
export function findConflicts(
  id: ShortcutId,
  binding: string,
  resolved: ResolvedBindings,
): ShortcutConflict[] {
  const { context } = shortcutAction(id);
  const conflicts: ShortcutConflict[] = [];
  for (const other of SHORTCUT_IDS) {
    if (other === id) continue;
    const otherContext = SHORTCUT_ACTIONS[other].context;
    if (!contextsOverlap(context, otherContext)) continue;
    const clash = resolved[other].find(candidate =>
      bindingsClash(candidate, binding),
    );
    if (!clash) continue;
    conflicts.push({
      id: other,
      binding: clash,
      kind:
        context === otherContext ||
        (context !== 'global' && otherContext !== 'global')
          ? 'same'
          : 'shadow',
    });
  }
  return conflicts;
}

/** Shift com uma tecla que não escreve (Shift+Space, Shift+Enter) */
const isShiftedNamedKey = (stroke: string) => {
  const parts = stroke.split('+');
  return parts.includes('Shift') && parts[parts.length - 1].length > 1;
};

const reservedBindings = (mac: boolean) =>
  new Set(
    ['Mod+q', 'Mod+c', 'Mod+v', 'Mod+x', ...(mac ? ['Mod+h', 'Mod+m'] : [])]
      .map(binding => normalizeBinding(binding, mac))
      .filter(binding => binding !== null),
  );

/**
 * Por que a tecla não serve para a ação (ou `null` se serve). Num atalho que
 * vale com o foco num campo, uma letra sozinha seria texto: exige
 * modificadora ou tecla F. Os de copiar/colar/sair não se tomam.
 */
export function validateBinding(
  id: ShortcutId,
  binding: string,
  mac: boolean = isMac,
): string | null {
  if (reservedBindings(mac).has(binding)) return 'Atalho reservado do sistema.';
  const { allowWhileTyping, context } = shortcutAction(id);
  // Na paleta o foco está sempre no campo, mas as letras são da busca
  // e as setas/Enter/Tab, da lista: qualquer tecla com nome serve.
  if (!allowWhileTyping || context === 'palette') return null;
  const ok = bindingStrokes(binding).every(
    stroke =>
      hasModifier(stroke) || isFunctionKey(stroke) || isShiftedNamedKey(stroke),
  );
  return ok
    ? null
    : 'Este atalho vale enquanto se digita: use uma modificadora (⌘, Ctrl, Alt) ou uma tecla F.';
}

// ── Casamento com o evento ──────────────────────────────────────────────────

export type ShortcutMatch<Id extends string> =
  | { kind: 'fire'; id: Id }
  /** Começo de uma sequência: espera a próxima tecla */
  | { kind: 'pending'; sequence: string }
  | null;

/**
 * Qual das ações `ids` a tecla dispara, continuando a sequência pendente
 * (`pending`, as teclas já apertadas de um `g g`). Uma sequência ganha do
 * acorde simples; e a forma exata do evento, da leniente (Ctrl como ⌘ no
 * macOS). Se a tecla não continua a sequência, ela é tentada sozinha.
 */
export function matchShortcut<Id extends ShortcutId>(
  event: KeyEventLike,
  ids: readonly Id[],
  resolved: ResolvedBindings,
  pending: string | null,
  mac: boolean = isMac,
): ShortcutMatch<Id> {
  const strokes = [eventToStroke(event, mac), eventToStroke(event, mac, true)];
  const prefixes = pending ? [pending, null] : [null];

  for (const prefix of prefixes) {
    for (const stroke of strokes) {
      if (!stroke) continue;
      const sequence = prefix ? `${prefix} ${stroke}` : stroke;

      if (
        ids.some(id =>
          resolved[id].some(binding => binding.startsWith(`${sequence} `)),
        )
      ) {
        return { kind: 'pending', sequence };
      }
      const id = ids.find(id => resolved[id].includes(sequence));
      if (id) return { kind: 'fire', id };
    }
  }
  return null;
}
