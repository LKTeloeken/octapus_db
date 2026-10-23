import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import toast from 'react-hot-toast';
import {
  SHORTCUT_GROUPS,
  SHORTCUT_IDS,
  shortcutAction,
  type ShortcutContext,
  type ShortcutGroup,
  type ShortcutId,
} from '@/lib/keybindings/actions';
import {
  bindingStrokes,
  eventToStroke,
  formatBinding,
} from '@/lib/keybindings/chord';
import {
  defaultBindings,
  findConflicts,
  validateBinding,
  type ResolvedBindings,
} from '@/lib/keybindings/resolve';
import { SEQUENCE_WINDOW_MS } from '@/lib/keybindings/use-shortcuts';
import { useSaveKeybindings } from '@/queries/use-keybindings';
import { useKeybindingsStore } from '@/stores/keybindings-store';

/** Teclas de uma sequência gravada (`g g`) — mais que isso ninguém lembra */
const MAX_STROKES = 2;

export interface ShortcutBindingView {
  binding: string;
  label: string;
}

export interface ShortcutRowView {
  id: ShortcutId;
  label: string;
  bindings: ShortcutBindingView[];
  /** Difere do padrão */
  customized: boolean;
  /** Avisos de sombra: a mesma tecla vale para outra ação noutro painel */
  notes: string[];
}

export interface ShortcutGroupView {
  group: ShortcutGroup;
  rows: ShortcutRowView[];
}

/** A tecla já é de outra ação no mesmo lugar: substituir ou desistir */
export interface PendingConflict {
  id: ShortcutId;
  binding: string;
  message: string;
}

const describeAction = (id: ShortcutId) => {
  const { label, group } = shortcutAction(id);
  return `“${label}” (${group})`;
};

/** O painel de cada contexto (fora o global), para as notas de sombra */
const PANEL: Record<Exclude<ShortcutContext, 'global'>, [string, string]> = {
  assistant: ['no', 'assistente'],
  table: ['na', 'aba de tabela'],
  grid: ['na', 'grade'],
  tree: ['na', 'árvore'],
  palette: ['na', 'paleta'],
  editor: ['no', 'editor SQL'],
};

/**
 * A mesma tecla numa ação global e numa de painel: com o foco no painel vale
 * a dele, fora dele a global (`Com o foco no assistente, ⌘W faz “Fechar a
 * conversa”`).
 */
const shadowNote = (id: ShortcutId, otherId: ShortcutId, binding: string) => {
  const key = formatBinding(binding);
  const other = shortcutAction(otherId);
  if (other.context === 'global') {
    const own = shortcutAction(id).context as Exclude<
      ShortcutContext,
      'global'
    >;
    const [article, panel] = PANEL[own];
    const from = article === 'no' ? 'do' : 'da';
    return `Com o foco fora ${from} ${panel}, ${key} faz “${other.label}”.`;
  }
  const [article, panel] = PANEL[other.context];
  return `Com o foco ${article} ${panel}, ${key} faz “${other.label}”.`;
};

const normalizeText = (text: string) =>
  text.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '');

export const useShortcutsSection = () => {
  const resolved = useKeybindingsStore(state => state.resolved);
  const overrides = useKeybindingsStore(state => state.overrides);
  const setRecordingFlag = useKeybindingsStore(state => state.setRecording);
  const { save, resetAll, isSaving } = useSaveKeybindings();

  const [query, setQuery] = useState('');
  const [recording, setRecording] = useState<{
    id: ShortcutId;
    strokes: string[];
  } | null>(null);
  const [conflict, setConflict] = useState<PendingConflict | null>(null);
  const [error, setError] = useState<{
    id: ShortcutId;
    message: string;
  } | null>(null);

  const persist = useCallback(
    (changes: Partial<ResolvedBindings>) =>
      save(changes, {
        onError: failure =>
          toast.error(
            failure instanceof Error ? failure.message : String(failure),
          ),
      }),
    [save],
  );

  // ── Gravação ──────────────────────────────────────────────────────────

  const stopRecording = useCallback(() => {
    setRecording(null);
    setRecordingFlag(false);
  }, [setRecordingFlag]);

  const startRecording = useCallback(
    (id: ShortcutId) => {
      setConflict(null);
      setError(null);
      setRecording({ id, strokes: [] });
      setRecordingFlag(true);
    },
    [setRecordingFlag],
  );

  // A seção fechou no meio da gravação: os atalhos voltam a valer.
  useEffect(() => () => setRecordingFlag(false), [setRecordingFlag]);

  const commit = useCallback(
    (id: ShortcutId, binding: string) => {
      stopRecording();
      if (resolved[id].includes(binding)) return;

      const invalid = validateBinding(id, binding);
      if (invalid) {
        setError({ id, message: invalid });
        return;
      }

      const same = findConflicts(id, binding, resolved).filter(
        item => item.kind === 'same',
      );
      if (same.length > 0) {
        setConflict({
          id,
          binding,
          message: `${formatBinding(binding)} já é de ${same
            .map(item => describeAction(item.id))
            .join(', ')}.`,
        });
        return;
      }

      persist({ [id]: [...resolved[id], binding] });
    },
    [resolved, persist, stopRecording],
  );

  // Enquanto grava, toda tecla é do gravador: na captura do `window` e sem
  // propagar — nem o Esc chega ao diálogo, nem o Tab tira o foco.
  const recordingRef = useRef(recording);
  recordingRef.current = recording;
  const active = recording !== null;
  useEffect(() => {
    if (!active) return;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopImmediatePropagation();
      const current = recordingRef.current;
      if (!current) return;

      const stroke = eventToStroke(event);
      if (stroke === null) return; // só a modificadora, ainda
      if (stroke === 'Escape' && current.strokes.length === 0) {
        stopRecording();
        return;
      }

      const strokes = [...current.strokes, stroke];
      clearTimeout(timer);
      if (strokes.length >= MAX_STROKES) {
        commit(current.id, strokes.join(' '));
        return;
      }
      setRecording({ id: current.id, strokes });
      // Outra tecla logo em seguida vira sequência (`g g`)
      timer = setTimeout(
        () => commit(current.id, strokes.join(' ')),
        SEQUENCE_WINDOW_MS,
      );
    };

    window.addEventListener('keydown', onKeyDown, true);
    return () => {
      clearTimeout(timer);
      window.removeEventListener('keydown', onKeyDown, true);
    };
  }, [active, commit, stopRecording]);

  // ── Edição ────────────────────────────────────────────────────────────

  const replaceConflict = useCallback(() => {
    if (!conflict) return;
    const { id, binding } = conflict;
    const changes: Partial<ResolvedBindings> = {
      [id]: [...resolved[id], binding],
    };
    for (const other of findConflicts(id, binding, resolved)) {
      if (other.kind !== 'same') continue;
      changes[other.id] = resolved[other.id].filter(
        item => item !== other.binding,
      );
    }
    persist(changes);
    setConflict(null);
  }, [conflict, resolved, persist]);

  const removeBinding = useCallback(
    (id: ShortcutId, binding: string) => {
      setError(null);
      persist({ [id]: resolved[id].filter(item => item !== binding) });
    },
    [resolved, persist],
  );

  const resetAction = useCallback(
    (id: ShortcutId) => {
      setError(null);
      setConflict(null);
      persist({ [id]: defaultBindings(id) });
    },
    [persist],
  );

  const restoreAll = useCallback(() => {
    setError(null);
    setConflict(null);
    resetAll({
      onError: failure =>
        toast.error(
          failure instanceof Error ? failure.message : String(failure),
        ),
    });
  }, [resetAll]);

  // ── O que a tela mostra ───────────────────────────────────────────────

  const groups = useMemo<ShortcutGroupView[]>(() => {
    const needle = normalizeText(query.trim());
    const rows = SHORTCUT_IDS.map((id): ShortcutRowView => {
      const bindings = resolved[id].map(binding => ({
        binding,
        label: formatBinding(binding),
      }));
      const notes = resolved[id].flatMap(binding =>
        findConflicts(id, binding, resolved)
          .filter(item => item.kind === 'shadow')
          .map(item => shadowNote(id, item.id, binding)),
      );
      return {
        id,
        label: shortcutAction(id).label,
        bindings,
        customized: Object.prototype.hasOwnProperty.call(overrides, id),
        notes,
      };
    });

    const matches = (row: ShortcutRowView) =>
      !needle ||
      normalizeText(
        `${row.label} ${shortcutAction(row.id).group} ${row.bindings
          .map(binding => `${binding.label} ${binding.binding}`)
          .join(' ')}`,
      ).includes(needle);

    return SHORTCUT_GROUPS.map(group => ({
      group,
      rows: rows.filter(
        row => shortcutAction(row.id).group === group && matches(row),
      ),
    })).filter(group => group.rows.length > 0);
  }, [query, resolved, overrides]);

  const recordingLabel = recording
    ? recording.strokes.length > 0
      ? bindingStrokes(recording.strokes.join(' '))
          .map(stroke => formatBinding(stroke))
          .join(' ')
      : null
    : null;

  return {
    query,
    setQuery,
    groups,
    hasCustomizations: Object.keys(overrides).length > 0,
    isSaving,
    recordingId: recording?.id ?? null,
    recordingLabel,
    startRecording,
    cancelRecording: stopRecording,
    conflict,
    replaceConflict,
    dismissConflict: () => setConflict(null),
    error,
    removeBinding,
    resetAction,
    restoreAll,
  };
};
