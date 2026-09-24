import { memo, useMemo } from 'react';
import CodeMirror from '@uiw/react-codemirror';
import { oneDark } from '@codemirror/theme-one-dark';
import { html } from '@codemirror/lang-html';
import { json } from '@codemirror/lang-json';
import { xml } from '@codemirror/lang-xml';
import type { Extension } from '@codemirror/state';
import { EditorView, keymap, placeholder } from '@codemirror/view';
import type { ValueFormat } from '@/lib/value-format';
import { useUiStore } from '@/stores/ui-store';

const disableSpellcheck = EditorView.contentAttributes.of({
  spellcheck: 'false',
  autocorrect: 'off',
  autocapitalize: 'off',
});

const languageFor = (format: ValueFormat): Extension[] => {
  switch (format) {
    case 'json':
      return [json()];
    case 'xml':
      return [xml()];
    case 'html':
      return [html()];
    default:
      return [];
  }
};

interface ValueEditorProps {
  value: string;
  format: ValueFormat;
  wordWrap: boolean;
  readOnly: boolean;
  /** Célula NULL: o editor vazio mostra "NULL" em vez de parecer string vazia */
  isNull: boolean;
  onChange: (value: string) => void;
  onEscape?: () => void;
}

export const ValueEditor = memo(
  ({
    value,
    format,
    wordWrap,
    readOnly,
    isNull,
    onChange,
    onEscape,
  }: ValueEditorProps) => {
    const theme = useUiStore(state => state.theme);

    const extensions = useMemo(() => {
      const list: Extension[] = [disableSpellcheck, ...languageFor(format)];
      // O dump binário tem colunas alinhadas: quebrar a linha o desmonta.
      if (wordWrap && format !== 'binary') list.push(EditorView.lineWrapping);
      if (isNull) list.push(placeholder('NULL'));
      if (onEscape) {
        list.push(
          keymap.of([
            {
              key: 'Escape',
              run: () => {
                onEscape();
                return true;
              },
            },
          ]),
        );
      }
      return list;
    }, [format, wordWrap, isNull, onEscape]);

    return (
      <CodeMirror
        className="h-full"
        height="100%"
        value={value}
        theme={theme === 'dark' ? oneDark : 'light'}
        extensions={extensions}
        // Só `readOnly`, sem `editable={false}`: continua dando para focar,
        // selecionar pelo teclado, buscar (Cmd/Ctrl+F) e copiar.
        readOnly={readOnly}
        basicSetup={{
          // No dump binário a coluna de offset já numera as linhas.
          lineNumbers: format !== 'binary',
          foldGutter: true,
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          bracketMatching: true,
          closeBrackets: true,
          autocompletion: false,
          rectangularSelection: false,
          crosshairCursor: false,
          highlightSelectionMatches: false,
          searchKeymap: true,
          defaultKeymap: true,
          history: true,
          drawSelection: true,
          indentOnInput: true,
          syntaxHighlighting: true,
        }}
        onChange={onChange}
        style={{ height: '100%', fontSize: 12 }}
      />
    );
  },
);

ValueEditor.displayName = 'ValueEditor';
