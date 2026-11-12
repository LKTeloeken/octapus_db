import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorState, type Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { tags as t } from '@lezer/highlight';

/**
 * Tema do CodeMirror derivado dos tokens do Ink (DESIGN.md §2.7): as cores são
 * `var(--…)`, então o mesmo tema acompanha claro/escuro sozinho — o `dark` só
 * ajusta os padrões internos do CodeMirror. Substitui o `oneDark`.
 */
// Mesma pilha do `--font-mono` do tema (o Tailwind não publica a variável).
const MONO_FONT = "'Geist Mono Variable', ui-monospace, monospace";
// Os painéis (busca, ir para a linha) são UI do app, não código: a pilha do
// `--font-sans` (que o Tailwind também não publica).
const SANS_FONT = "'Geist Variable', ui-sans-serif, system-ui, sans-serif";

/**
 * Rótulos dos painéis do CodeMirror (busca do Cmd/Ctrl+F, ir para a linha) em
 * português, como o resto do app.
 */
const PHRASES = {
  Find: 'Buscar',
  Replace: 'Substituir por',
  next: 'Próxima',
  previous: 'Anterior',
  all: 'Selecionar todas',
  'match case': 'Maiúsculas',
  'by word': 'Palavra inteira',
  regexp: 'Regex',
  replace: 'Substituir',
  'replace all': 'Substituir todas',
  close: 'Fechar',
  'current match': 'ocorrência atual',
  'replaced $ matches': '$ ocorrências substituídas',
  'replaced match on line $': 'ocorrência substituída na linha $',
  'on line': 'na linha',
  'Go to line': 'Ir para a linha',
  go: 'Ir',
};

const editorChrome = (dark: boolean) =>
  EditorView.theme(
    {
      '&': {
        color: 'var(--fg)',
        backgroundColor: 'transparent',
        height: '100%',
      },
      '&.cm-focused': { outline: 'none' },
      '.cm-scroller': {
        fontFamily: MONO_FONT,
        lineHeight: '22px',
      },
      '.cm-content': {
        caretColor: 'var(--iris-text)',
        padding: '10px 0',
      },
      '.cm-cursor, .cm-dropCursor': {
        borderLeftColor: 'var(--iris-text)',
        borderLeftWidth: '2px',
      },
      '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection':
        { backgroundColor: 'var(--iris-soft)' },
      '.cm-activeLine': { backgroundColor: 'var(--hover)' },
      '.cm-selectionMatch': { backgroundColor: 'var(--active)' },
      '&.cm-focused .cm-matchingBracket': {
        backgroundColor: 'var(--active)',
        outline: '1px solid var(--line-strong)',
      },
      '.cm-gutters': {
        backgroundColor: 'transparent',
        // Números de linha são conteúdo: --fg-subtle (AA), não --fg-disabled.
        color: 'var(--fg-subtle)',
        border: 'none',
      },
      '.cm-lineNumbers .cm-gutterElement': {
        padding: '0 12px 0 16px',
        minWidth: '44px',
      },
      '.cm-activeLineGutter': {
        backgroundColor: 'transparent',
        color: 'var(--fg)',
      },
      '.cm-foldPlaceholder': {
        backgroundColor: 'var(--active)',
        border: 'none',
        color: 'var(--fg-muted)',
      },
      '.cm-placeholder': { color: 'var(--fg-subtle)' },
      // Popups (autocomplete, info): sólidos como os menus do app. Moram no `body`
      // (query-editor.tsx), então o tamanho da fonte não vem mais do editor.
      '.cm-tooltip': {
        fontSize: '13px',
        backgroundColor: 'var(--surface-3)',
        color: 'var(--fg)',
        border: '1px solid var(--line)',
        borderRadius: '12px',
        boxShadow: 'var(--elev-2)',
        overflow: 'hidden',
      },
      '.cm-tooltip-autocomplete > ul': {
        fontFamily: MONO_FONT,
        padding: '4px',
      },
      '.cm-tooltip-autocomplete > ul > li': {
        borderRadius: '6px',
        padding: '2px 8px',
        lineHeight: '22px',
      },
      '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
        backgroundColor: 'var(--active)',
        color: 'var(--fg)',
      },
      '.cm-completionDetail': {
        color: 'var(--fg-subtle)',
        fontStyle: 'normal',
      },
      '.cm-completionMatchedText': {
        textDecoration: 'none',
        color: 'var(--iris-text)',
        fontWeight: '600',
      },
      '.cm-completionIcon': { opacity: '0.7' },
      '.cm-panels': {
        backgroundColor: 'var(--surface-2)',
        color: 'var(--fg)',
      },
      '.cm-panels.cm-panels-top': { borderBottom: '1px solid var(--line)' },
      '.cm-panels.cm-panels-bottom': { borderTop: '1px solid var(--line)' },
      // Busca (Cmd/Ctrl+F) e "ir para a linha": campos e botões no padrão dos
      // controles do app (28 px, `--control`), não os nativos do CodeMirror.
      '.cm-panel.cm-search, .cm-panel.cm-gotoLine': {
        position: 'relative',
        // À direita, o espaço do botão de fechar
        padding: '8px 40px 2px 10px',
        fontFamily: SANS_FONT,
        fontSize: '13px',
        lineHeight: '20px',
        color: 'var(--fg)',
      },
      '.cm-panel .cm-textfield': {
        boxSizing: 'border-box',
        height: '28px',
        minWidth: '180px',
        margin: '0 6px 6px 0',
        padding: '0 8px',
        verticalAlign: 'middle',
        fontFamily: MONO_FONT,
        fontSize: '13px',
        color: 'var(--fg)',
        backgroundColor: 'var(--surface-1)',
        border: '1px solid var(--control-border)',
        borderRadius: 'var(--radius-sm)',
        outline: 'none',
      },
      '.cm-panel .cm-textfield::placeholder': { color: 'var(--fg-subtle)' },
      '.cm-panel .cm-textfield:focus-visible': {
        outline: '2px solid var(--ring)',
        outlineOffset: '-1px',
      },
      '.cm-panel .cm-button': {
        boxSizing: 'border-box',
        height: '28px',
        margin: '0 6px 6px 0',
        padding: '0 10px',
        verticalAlign: 'middle',
        fontFamily: SANS_FONT,
        fontSize: '13px',
        color: 'var(--fg)',
        // O tema base do CodeMirror pinta um gradiente por cima
        backgroundImage: 'none',
        backgroundColor: 'var(--control)',
        border: '1px solid var(--control-border)',
        borderRadius: 'var(--radius-sm)',
        boxShadow: 'var(--control-shadow)',
        cursor: 'pointer',
      },
      '.cm-panel .cm-button:hover, .cm-panel .cm-button:active': {
        backgroundImage: 'none',
        backgroundColor: 'var(--active)',
      },
      '.cm-panel .cm-button:focus-visible, .cm-panel button[name=close]:focus-visible':
        {
          outline: '2px solid var(--ring)',
          outlineOffset: '1px',
        },
      '.cm-panel.cm-search label': {
        display: 'inline-flex',
        alignItems: 'center',
        gap: '6px',
        height: '28px',
        margin: '0 12px 6px 0',
        verticalAlign: 'middle',
        fontSize: '12px',
        color: 'var(--fg-muted)',
        cursor: 'pointer',
      },
      // Caixa própria: a nativa fica branca no tema escuro
      '.cm-panel.cm-search input[type=checkbox]': {
        appearance: 'none',
        boxSizing: 'border-box',
        width: '14px',
        height: '14px',
        margin: '0',
        backgroundColor: 'var(--surface-1)',
        border: '1px solid var(--control-border)',
        borderRadius: 'var(--radius-xs)',
        cursor: 'pointer',
      },
      '.cm-panel.cm-search input[type=checkbox]:checked': {
        backgroundColor: 'var(--iris)',
        borderColor: 'var(--iris)',
        // Miolo vazado no lugar do "✓": só tokens, sem imagem
        boxShadow: 'inset 0 0 0 2px var(--surface-1)',
      },
      '.cm-panel.cm-search input[type=checkbox]:focus-visible': {
        outline: '2px solid var(--ring)',
        outlineOffset: '1px',
      },
      '.cm-panel.cm-search button[name=close], .cm-panel.cm-gotoLine button[name=close]':
        {
          position: 'absolute',
          top: '8px',
          right: '8px',
          width: '28px',
          height: '28px',
          padding: '0',
          fontSize: '18px',
          lineHeight: '1',
          color: 'var(--fg-muted)',
          backgroundColor: 'transparent',
          border: 'none',
          borderRadius: 'var(--radius-sm)',
          cursor: 'pointer',
        },
      '.cm-panel.cm-search button[name=close]:hover, .cm-panel.cm-gotoLine button[name=close]:hover':
        {
          color: 'var(--fg)',
          backgroundColor: 'var(--hover)',
        },
      '.cm-searchMatch': {
        backgroundColor: 'var(--warning-soft)',
        outline: '1px solid var(--warning)',
      },
      '.cm-searchMatch.cm-searchMatch-selected': {
        backgroundColor: 'var(--iris-soft)',
      },
    },
    { dark },
  );

const inkHighlight = HighlightStyle.define([
  {
    tag: [t.keyword, t.operatorKeyword, t.modifier, t.bool, t.null],
    color: 'var(--syn-keyword)',
  },
  {
    tag: [
      t.function(t.variableName),
      t.function(t.propertyName),
      t.standard(t.name),
    ],
    color: 'var(--syn-function)',
  },
  {
    tag: [t.string, t.special(t.string), t.regexp],
    color: 'var(--syn-string)',
  },
  { tag: [t.number, t.integer, t.float], color: 'var(--syn-number)' },
  { tag: [t.typeName, t.className, t.namespace], color: 'var(--syn-type)' },
  {
    tag: [t.comment, t.lineComment, t.blockComment],
    color: 'var(--fg-subtle)',
    fontStyle: 'italic',
  },
  {
    tag: [t.operator, t.punctuation, t.separator, t.bracket],
    color: 'var(--fg-muted)',
  },
  { tag: [t.propertyName, t.attributeName], color: 'var(--fg)' },
  { tag: [t.tagName], color: 'var(--syn-keyword)' },
  { tag: [t.invalid], color: 'var(--danger)' },
]);

/** Tema completo (moldura + realce de sintaxe) para o `theme` do `@uiw/react-codemirror`. */
export const inkEditorTheme = (dark: boolean): Extension => [
  editorChrome(dark),
  syntaxHighlighting(inkHighlight),
  EditorState.phrases.of(PHRASES),
];
