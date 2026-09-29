import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { tags as t } from '@lezer/highlight';

/**
 * Tema do CodeMirror derivado dos tokens do Ink (DESIGN.md §2.7): as cores são
 * `var(--…)`, então o mesmo tema acompanha claro/escuro sozinho — o `dark` só
 * ajusta os padrões internos do CodeMirror. Substitui o `oneDark`.
 */
// Mesma pilha do `--font-mono` do tema (o Tailwind não publica a variável).
const MONO_FONT = "'Geist Mono Variable', ui-monospace, monospace";

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
      // Popups (autocomplete, info): sólidos como os menus do app.
      '.cm-tooltip': {
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
      '.cm-completionDetail': { color: 'var(--fg-subtle)', fontStyle: 'normal' },
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
  { tag: [t.keyword, t.operatorKeyword, t.modifier, t.bool, t.null], color: 'var(--syn-keyword)' },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.standard(t.name)], color: 'var(--syn-function)' },
  { tag: [t.string, t.special(t.string), t.regexp], color: 'var(--syn-string)' },
  { tag: [t.number, t.integer, t.float], color: 'var(--syn-number)' },
  { tag: [t.typeName, t.className, t.namespace], color: 'var(--syn-type)' },
  { tag: [t.comment, t.lineComment, t.blockComment], color: 'var(--fg-subtle)', fontStyle: 'italic' },
  { tag: [t.operator, t.punctuation, t.separator, t.bracket], color: 'var(--fg-muted)' },
  { tag: [t.propertyName, t.attributeName], color: 'var(--fg)' },
  { tag: [t.tagName], color: 'var(--syn-keyword)' },
  { tag: [t.invalid], color: 'var(--danger)' },
]);

/** Tema completo (moldura + realce de sintaxe) para o `theme` do `@uiw/react-codemirror`. */
export const inkEditorTheme = (dark: boolean): Extension => [
  editorChrome(dark),
  syntaxHighlighting(inkHighlight),
];
