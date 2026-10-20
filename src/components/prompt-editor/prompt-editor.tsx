import { useEffect, useMemo, useRef } from 'react';
import CodeMirror, { type ReactCodeMirrorRef } from '@uiw/react-codemirror';
import {
  autocompletion,
  currentCompletions,
  type CompletionSource,
} from '@codemirror/autocomplete';
import { Prec } from '@codemirror/state';
import {
  Decoration,
  EditorView,
  keymap,
  MatchDecorator,
  placeholder,
  tooltips,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from '@codemirror/view';
import { inkEditorTheme } from '@/lib/codemirror-theme';
import { useUiStore } from '@/stores/ui-store';

export interface PromptEditorProps {
  value: string;
  onChange: (value: string) => void;
  /** Enter envia; Shift+Enter quebra a linha. Com o popup aberto, Enter é dele */
  onSubmit: () => void;
  /** Esc com o popup fechado (ex.: cancelar o turno em andamento) */
  onEscape?: () => void;
  /** Sugestões (menções); precisa de identidade estável */
  completionSource?: CompletionSource;
  /** Trechos destacados como menção (`@schema.tabela`) */
  mentionPattern?: RegExp;
  placeholderText?: string;
  disabled?: boolean;
  /** Muda o número → o campo ganha foco */
  focusRequest?: number;
  ariaLabel: string;
  className?: string;
}

/** Popups no `body`, como no editor SQL: fora do `overflow: hidden` do painel */
const bodyTooltips = tooltips({
  parent: typeof document === 'undefined' ? undefined : document.body,
});

/**
 * Texto corrido, não código: fonte da interface, quebra de linha, sem gutter.
 * `Prec.highest` porque o tema do Ink (prop `theme`) entra depois.
 */
const proseTheme = Prec.highest(
  EditorView.theme({
    '.cm-scroller': {
      fontFamily: 'inherit',
      lineHeight: '20px',
      maxHeight: '200px',
      overflowY: 'auto',
    },
    '.cm-content': { padding: '8px 0' },
    '.cm-line': { padding: '0 12px' },
    '.cm-mention': {
      backgroundColor: 'var(--active)',
      color: 'var(--fg)',
      borderRadius: '4px',
      padding: '0 2px',
      fontFamily: "'Geist Mono Variable', ui-monospace, monospace",
      fontSize: '12px',
    },
  }),
);

const mentionMark = Decoration.mark({ class: 'cm-mention' });

function mentionHighlighter(pattern: RegExp) {
  const decorator = new MatchDecorator({
    regexp: pattern,
    decoration: () => mentionMark,
  });
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = decorator.createDeco(view);
      }
      update(update: ViewUpdate) {
        this.decorations = decorator.updateDeco(update, this.decorations);
      }
    },
    { decorations: plugin => plugin.decorations },
  );
}

/**
 * Campo de pergunta em linguagem natural (assistente). CodeMirror pelo
 * autocomplete com teclado e pelo destaque das menções; o resto é um campo de
 * texto de várias linhas.
 */
export function PromptEditor({
  value,
  onChange,
  onSubmit,
  onEscape,
  completionSource,
  mentionPattern,
  placeholderText = '',
  disabled = false,
  focusRequest,
  ariaLabel,
  className,
}: PromptEditorProps) {
  const appTheme = useUiStore(state => state.theme);
  const editorTheme = useMemo(
    () => inkEditorTheme(appTheme === 'dark'),
    [appTheme],
  );
  const ref = useRef<ReactCodeMirrorRef>(null);

  // Adaptadores de identidade fixa: as props podem mudar sem reconfigurar
  const submitRef = useRef(onSubmit);
  submitRef.current = onSubmit;
  const escapeRef = useRef(onEscape);
  escapeRef.current = onEscape;
  const sourceRef = useRef(completionSource);
  sourceRef.current = completionSource;

  const extensions = useMemo(
    () => [
      EditorView.lineWrapping,
      EditorView.contentAttributes.of({
        'aria-label': ariaLabel,
        'aria-multiline': 'true',
        spellcheck: 'true',
      }),
      proseTheme,
      bodyTooltips,
      placeholder(placeholderText),
      ...(mentionPattern ? [mentionHighlighter(mentionPattern)] : []),
      autocompletion({
        override: [context => sourceRef.current?.(context) ?? null],
        activateOnTyping: true,
        icons: false,
        closeOnBlur: true,
      }),
      // Esc na frente do autocomplete: com opções na tela, fecha o popup;
      // sem, é do campo (ver o mesmo caso no QueryEditor)
      Prec.highest(
        keymap.of([
          {
            key: 'Escape',
            run: view => {
              if (currentCompletions(view.state).length > 0) return false;
              if (!escapeRef.current) return false;
              escapeRef.current();
              return true;
            },
          },
        ]),
      ),
      // Abaixo do keymap do autocomplete: com o popup aberto, Enter aceita
      Prec.high(
        keymap.of([
          {
            key: 'Enter',
            run: () => {
              submitRef.current();
              return true;
            },
          },
        ]),
      ),
    ],
    [ariaLabel, placeholderText, mentionPattern],
  );

  // O `@uiw/react-codemirror` segura valor externo enquanto a pessoa digita
  // (trava de ~200 ms): limpar o campo ao enviar esperaria o próximo render.
  // Aqui o valor da prop vale na hora.
  useEffect(() => {
    const view = ref.current?.view;
    if (!view || view.state.doc.toString() === value) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: value },
    });
  }, [value]);

  // O `@uiw/react-codemirror` cria a view depois do primeiro render: um pedido
  // de foco que chega antes dela é atendido no `onCreateEditor`
  const wantsFocus = useRef(focusRequest !== undefined);
  useEffect(() => {
    if (focusRequest === undefined || disabled) return;
    const view = ref.current?.view;
    if (view) view.focus();
    else wantsFocus.current = true;
  }, [focusRequest, disabled]);

  return (
    <div className={className}>
      <CodeMirror
        ref={ref}
        value={value}
        onChange={onChange}
        onCreateEditor={view => {
          if (wantsFocus.current && !disabled) view.focus();
          wantsFocus.current = false;
        }}
        theme={editorTheme}
        extensions={extensions}
        editable={!disabled}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          bracketMatching: false,
          closeBrackets: false,
          autocompletion: false,
          highlightSelectionMatches: false,
          searchKeymap: false,
          indentOnInput: false,
          syntaxHighlighting: false,
        }}
        style={{ fontSize: 13 }}
      />
    </div>
  );
}
