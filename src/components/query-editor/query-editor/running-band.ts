import { RangeSetBuilder, StateEffect, StateField } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';

/**
 * Faixa biolume na instrução que está rodando (DESIGN.md, Editor): as linhas
 * do trecho executado ganham `biolume-soft` e uma pílula de 3 px na margem,
 * até a consulta terminar. Segue as edições feitas enquanto roda.
 */
export const setRunningRange = StateEffect.define<{
  from: number;
  to: number;
} | null>();

const runningLine = Decoration.line({ class: 'cm-running-line' });

function decorate(
  doc: EditorView['state']['doc'],
  from: number,
  to: number,
): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  const first = doc.lineAt(Math.min(from, doc.length)).number;
  const last = doc.lineAt(Math.min(to, doc.length)).number;
  for (let number = first; number <= last; number++) {
    const line = doc.line(number);
    builder.add(line.from, line.from, runningLine);
  }
  return builder.finish();
}

export const runningBandField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(value, transaction) {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (!effect.is(setRunningRange)) continue;
      next = effect.value
        ? decorate(transaction.state.doc, effect.value.from, effect.value.to)
        : Decoration.none;
    }
    return next;
  },
  provide: field => EditorView.decorations.from(field),
});

export const runningBandTheme = EditorView.baseTheme({
  '.cm-running-line': {
    backgroundColor: 'var(--biolume-soft)',
    boxShadow: 'inset 3px 0 0 var(--biolume)',
  },
});

export const runningBand = [runningBandField, runningBandTheme];
