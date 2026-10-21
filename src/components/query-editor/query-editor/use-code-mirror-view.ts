import { useLayoutEffect, useMemo, useRef } from 'react';
import {
  Annotation,
  EditorState,
  StateEffect,
  type Extension,
} from '@codemirror/state';
import { EditorView } from '@codemirror/view';

/** Troca de texto vinda de fora (prop `value`): não volta pelo `onChange`. */
const externalChange = Annotation.define<boolean>();

interface StoredDocument {
  state: EditorState;
  scroll: StateEffect<unknown>;
}

/**
 * Estado de cada documento fora da tela, por id: texto, seleção, histórico de undo e
 * scroll. Um só `EditorView` atende todas as abas de query; trocar de aba troca o
 * `EditorState` inteiro (`setState`), em vez de reescrever o texto — reescrever mapeava a
 * seleção de uma aba na outra e misturava o undo das duas.
 */
const documents = new Map<string, StoredDocument>();

/** Descarta o estado guardado de um documento (aba fechada). */
export function forgetEditorDocument(id: string): void {
  documents.delete(id);
}

function replaceAll(state: EditorState, value: string): EditorState {
  return state.update({
    changes: { from: 0, to: state.doc.length, insert: value },
    annotations: externalChange.of(true),
  }).state;
}

/** Estado para abrir o documento: o guardado (com a configuração atual) ou um novo. */
function openDocument(
  id: string | undefined,
  value: string,
  extensions: Extension,
): StoredDocument | { state: EditorState; scroll?: undefined } {
  const stored = id === undefined ? undefined : documents.get(id);
  if (!stored) return { state: EditorState.create({ doc: value, extensions }) };

  // O reconfigure mantém texto, seleção e histórico (os campos continuam na config).
  let state = stored.state.update({
    effects: StateEffect.reconfigure.of(extensions),
  }).state;
  // O conteúdo mudou enquanto a aba estava fora (assistente, sessão restaurada).
  if (state.doc.toString() !== value) state = replaceAll(state, value);

  return { state, scroll: stored.scroll };
}

function storeDocument(id: string | undefined, view: EditorView): void {
  if (id === undefined) return;
  documents.set(id, { state: view.state, scroll: view.scrollSnapshot() });
}

interface UseCodeMirrorViewOptions {
  value: string;
  onChange: (value: string) => void;
  extensions: Extension[];
  /** Sem id, o editor tem um documento só e nada é guardado. */
  documentId?: string;
  autoFocus?: boolean;
}

/**
 * Monta o `EditorView` direto, sem o componente do `@uiw/react-codemirror`: ele criava a
 * view e sincronizava o `value` em `useEffect` (um quadro vazio ao montar) e segurava a
 * troca de texto por até 200 ms depois da última tecla. Aqui tudo roda em
 * `useLayoutEffect`, antes da pintura.
 */
export function useCodeMirrorView({
  value,
  onChange,
  extensions,
  documentId,
  autoFocus = false,
}: UseCodeMirrorViewOptions) {
  const parentRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  // Lidos pelos efeitos sem entrar nas dependências deles.
  const latest = useRef({ value, documentId, autoFocus });
  latest.current = { value, documentId, autoFocus };

  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  // Identidade fixa: trocar o `onChange` (outra aba) não reconfigura o editor.
  const changeListener = useMemo(
    () =>
      EditorView.updateListener.of(update => {
        if (!update.docChanged) return;
        if (update.transactions.some(tr => tr.annotation(externalChange))) {
          return;
        }
        onChangeRef.current(update.state.doc.toString());
      }),
    [],
  );

  const allExtensions = useMemo(
    () => [changeListener, ...extensions],
    [changeListener, extensions],
  );
  const extensionsRef = useRef(allExtensions);
  extensionsRef.current = allExtensions;

  // Documento que a view mostra agora.
  const shownDocumentId = useRef<string | undefined>(undefined);

  useLayoutEffect(() => {
    const parent = parentRef.current;
    if (!parent) return;

    const { value, documentId, autoFocus } = latest.current;
    const { state, scroll } = openDocument(
      documentId,
      value,
      extensionsRef.current,
    );
    const view = new EditorView({ state, parent });
    if (scroll) view.dispatch({ effects: scroll });
    if (autoFocus) view.focus();

    viewRef.current = view;
    shownDocumentId.current = documentId;

    return () => {
      // Sair para uma aba de tabela desmonta o editor: guarda para a volta.
      storeDocument(shownDocumentId.current, view);
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  // Outra aba: guarda o documento que sai e mostra o que entra.
  useLayoutEffect(() => {
    const view = viewRef.current;
    if (!view || shownDocumentId.current === documentId) return;

    storeDocument(shownDocumentId.current, view);
    const { state, scroll } = openDocument(
      documentId,
      latest.current.value,
      extensionsRef.current,
    );
    view.setState(state);
    if (scroll) view.dispatch({ effects: scroll });
    shownDocumentId.current = documentId;
  }, [documentId]);

  useLayoutEffect(() => {
    viewRef.current?.dispatch({
      effects: StateEffect.reconfigure.of(allExtensions),
    });
  }, [allExtensions]);

  // Texto alterado por fora no mesmo documento (inserir SQL, limpar o filtro).
  useLayoutEffect(() => {
    const view = viewRef.current;
    if (!view || view.state.doc.toString() === value) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: value },
      annotations: externalChange.of(true),
    });
  }, [value]);

  return { parentRef, viewRef };
}
