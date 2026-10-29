/**
 * O registro de todos os atalhos do app — a fonte única do que cada tecla
 * faz, dos padrões e do que aparece na seção Atalhos das configurações.
 *
 * Atalho novo: uma entrada aqui e, onde a ação acontece, um
 * `useShortcuts({ 'id.da.acao': handler })` (ver `use-shortcuts.ts`). Rótulo
 * de tecla na tela sai de `useShortcutLabel`, nunca escrito à mão.
 *
 * Teclas que não são atalho de verdade ficam de fora: Enter/Esc/Tab dentro dos
 * campos e editores de célula, as setas do SegmentedControl (acessibilidade)
 * e os atalhos nativos de texto (copiar, colar, desfazer num campo).
 */

/**
 * Onde a ação vale — define com quem uma tecla pode conflitar:
 * - `global`: em qualquer lugar do app;
 * - `assistant`: com o foco no painel do assistente;
 * - `table`: na aba de tabela (vale também com o foco na grade);
 * - `grid`, `tree`, `palette`, `editor`: com o foco nesse componente.
 */
export type ShortcutContext =
  | 'global'
  | 'assistant'
  | 'table'
  | 'grid'
  | 'tree'
  | 'palette'
  | 'editor';

export type ShortcutGroup =
  | 'Geral'
  | 'Abas'
  | 'Assistente'
  | 'Árvore'
  | 'Aba de tabela'
  | 'Grade'
  | 'Paleta de comandos'
  | 'Editor SQL';

export interface ShortcutAction {
  label: string;
  group: ShortcutGroup;
  context: ShortcutContext;
  /** Padrão; `{ mac, other }` quando muda com a plataforma */
  defaults: string[] | { mac: string[]; other: string[] };
  /**
   * Vale com o foco num campo de texto (input, textarea, CodeMirror)? Atalho
   * de letra sozinha nunca; acorde que o editor não usa, sim.
   */
  allowWhileTyping?: boolean;
}

const gotoTab = (n: number): ShortcutAction => ({
  label: `Ir para a aba ${n}`,
  group: 'Abas',
  context: 'global',
  defaults: [`Mod+${n}`],
  allowWhileTyping: true,
});

export const SHORTCUT_ACTIONS = {
  // ── Geral ───────────────────────────────────────────────────────────────
  'app.commandPalette': {
    label: 'Abrir a paleta de comandos',
    group: 'Geral',
    context: 'global',
    defaults: ['Mod+k'],
    allowWhileTyping: true,
  },
  // Nenhum editor do app usa esse acorde: vale até digitando
  'app.focusTree': {
    label: 'Focar a árvore de conexões',
    group: 'Geral',
    context: 'global',
    defaults: ['Mod+b'],
    allowWhileTyping: true,
  },
  // Num campo, Cmd/Ctrl+←/→ é "início/fim da linha"
  'app.focusLeft': {
    label: 'Ir para o painel da esquerda (árvore)',
    group: 'Geral',
    context: 'global',
    defaults: ['Mod+ArrowLeft'],
  },
  'app.focusRight': {
    label: 'Ir para o painel da direita (grade)',
    group: 'Geral',
    context: 'global',
    defaults: ['Mod+ArrowRight'],
  },

  // ── Abas ────────────────────────────────────────────────────────────────
  'tabs.close': {
    label: 'Fechar a aba',
    group: 'Abas',
    context: 'global',
    defaults: ['Mod+w'],
    allowWhileTyping: true,
  },
  'tabs.goto1': gotoTab(1),
  'tabs.goto2': gotoTab(2),
  'tabs.goto3': gotoTab(3),
  'tabs.goto4': gotoTab(4),
  'tabs.goto5': gotoTab(5),
  'tabs.goto6': gotoTab(6),
  'tabs.goto7': gotoTab(7),
  'tabs.goto8': gotoTab(8),
  'tabs.goto9': gotoTab(9),

  // ── Assistente ──────────────────────────────────────────────────────────
  'assistant.togglePanel': {
    label: 'Abrir ou fechar o assistente',
    group: 'Assistente',
    context: 'global',
    defaults: ['Mod+j'],
    allowWhileTyping: true,
  },
  'assistant.toggleAgentMode': {
    label: 'Entrar ou sair do modo agente',
    group: 'Assistente',
    context: 'global',
    defaults: ['Mod+Shift+j'],
    allowWhileTyping: true,
  },
  'assistant.newConversation': {
    label: 'Nova conversa',
    group: 'Assistente',
    context: 'assistant',
    defaults: ['Mod+t'],
    allowWhileTyping: true,
  },
  'assistant.closeConversation': {
    label: 'Fechar a conversa',
    group: 'Assistente',
    context: 'assistant',
    defaults: ['Mod+w'],
    allowWhileTyping: true,
  },

  // ── Árvore ──────────────────────────────────────────────────────────────
  'tree.down': {
    label: 'Descer',
    group: 'Árvore',
    context: 'tree',
    defaults: ['ArrowDown', 'j'],
  },
  'tree.up': {
    label: 'Subir',
    group: 'Árvore',
    context: 'tree',
    defaults: ['ArrowUp', 'k'],
  },
  'tree.expand': {
    label: 'Expandir (ou entrar no primeiro filho)',
    group: 'Árvore',
    context: 'tree',
    defaults: ['ArrowRight', 'l'],
  },
  'tree.collapse': {
    label: 'Recolher (ou subir ao pai)',
    group: 'Árvore',
    context: 'tree',
    defaults: ['ArrowLeft', 'h'],
  },
  'tree.open': {
    label: 'Abrir o item',
    group: 'Árvore',
    context: 'tree',
    defaults: ['Enter', 'o'],
  },
  'tree.first': {
    label: 'Ir ao primeiro item',
    group: 'Árvore',
    context: 'tree',
    defaults: ['g g'],
  },
  'tree.last': {
    label: 'Ir ao último item',
    group: 'Árvore',
    context: 'tree',
    defaults: ['Shift+g'],
  },
  'tree.filter': {
    label: 'Filtrar a lista',
    group: 'Árvore',
    context: 'tree',
    defaults: ['/'],
  },
  'tree.blur': {
    label: 'Sair da árvore',
    group: 'Árvore',
    context: 'tree',
    defaults: ['Escape'],
  },

  // ── Aba de tabela ───────────────────────────────────────────────────────
  'table.applyFilter': {
    label: 'Aplicar o filtro e recarregar',
    group: 'Aba de tabela',
    context: 'table',
    defaults: ['Mod+r'],
    allowWhileTyping: true,
  },
  'table.toggleValuePanel': {
    label: 'Mostrar ou esconder o painel de valor',
    group: 'Aba de tabela',
    context: 'table',
    defaults: ['Mod+i', 'F7'],
    allowWhileTyping: true,
  },

  // ── Grade ───────────────────────────────────────────────────────────────
  'grid.down': {
    label: 'Célula abaixo',
    group: 'Grade',
    context: 'grid',
    defaults: ['ArrowDown', 'j'],
  },
  'grid.up': {
    label: 'Célula acima',
    group: 'Grade',
    context: 'grid',
    defaults: ['ArrowUp', 'k'],
  },
  'grid.left': {
    label: 'Célula à esquerda',
    group: 'Grade',
    context: 'grid',
    defaults: ['ArrowLeft', 'h'],
  },
  'grid.right': {
    label: 'Célula à direita',
    group: 'Grade',
    context: 'grid',
    defaults: ['ArrowRight', 'l'],
  },
  'grid.rowStart': {
    label: 'Primeira coluna',
    group: 'Grade',
    context: 'grid',
    defaults: ['Home'],
  },
  'grid.rowEnd': {
    label: 'Última coluna',
    group: 'Grade',
    context: 'grid',
    defaults: ['End'],
  },
  // Shift+setas: estende a seleção de células a partir da célula de origem
  'grid.extendDown': {
    label: 'Estender a seleção para baixo',
    group: 'Grade',
    context: 'grid',
    defaults: ['Shift+ArrowDown'],
  },
  'grid.extendUp': {
    label: 'Estender a seleção para cima',
    group: 'Grade',
    context: 'grid',
    defaults: ['Shift+ArrowUp'],
  },
  'grid.extendLeft': {
    label: 'Estender a seleção para a esquerda',
    group: 'Grade',
    context: 'grid',
    defaults: ['Shift+ArrowLeft'],
  },
  'grid.extendRight': {
    label: 'Estender a seleção para a direita',
    group: 'Grade',
    context: 'grid',
    defaults: ['Shift+ArrowRight'],
  },
  'grid.edit': {
    label: 'Editar a célula',
    group: 'Grade',
    context: 'grid',
    defaults: ['Enter', 'Space'],
  },
  'grid.toggleView': {
    label: 'Alternar tabela e vertical',
    group: 'Grade',
    context: 'grid',
    defaults: ['Tab'],
  },
  'grid.save': {
    label: 'Salvar as alterações',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+s'],
    allowWhileTyping: true,
  },
  'grid.discardChanges': {
    label: 'Descartar as alterações',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+z'],
  },
  'grid.selectAll': {
    label: 'Selecionar todas as linhas',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+a'],
  },
  // Num campo de texto o Cmd/Ctrl+C/V continua nativo (sem allowWhileTyping)
  'grid.copy': {
    label: 'Copiar as linhas ou células selecionadas',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+c'],
  },
  'grid.paste': {
    label: 'Colar as linhas copiadas como novas',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+v'],
  },
  'grid.duplicateRows': {
    label: 'Duplicar as linhas selecionadas',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+d'],
  },
  'grid.deleteRows': {
    label: 'Remover as linhas selecionadas',
    group: 'Grade',
    context: 'grid',
    defaults: ['Backspace', 'Delete'],
  },
  'grid.clearSelection': {
    label: 'Limpar a seleção',
    group: 'Grade',
    context: 'grid',
    defaults: ['Escape'],
  },
  'grid.search': {
    label: 'Buscar na grade',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+f'],
    allowWhileTyping: true,
  },
  'grid.searchNext': {
    label: 'Próximo resultado da busca',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+g'],
    allowWhileTyping: true,
  },
  'grid.searchPrev': {
    label: 'Resultado anterior da busca',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+Shift+g'],
    allowWhileTyping: true,
  },
  'grid.columns': {
    label: 'Escolher as colunas visíveis',
    group: 'Grade',
    context: 'grid',
    defaults: ['Mod+Shift+h'],
    allowWhileTyping: true,
  },

  // ── Paleta (o foco fica sempre no campo de busca) ───────────────────────
  'palette.down': {
    label: 'Próximo item',
    group: 'Paleta de comandos',
    context: 'palette',
    defaults: ['ArrowDown'],
    allowWhileTyping: true,
  },
  'palette.up': {
    label: 'Item anterior',
    group: 'Paleta de comandos',
    context: 'palette',
    defaults: ['ArrowUp'],
    allowWhileTyping: true,
  },
  'palette.select': {
    label: 'Abrir o item',
    group: 'Paleta de comandos',
    context: 'palette',
    defaults: ['Enter'],
    allowWhileTyping: true,
  },
  'palette.pinSchema': {
    label: 'Fixar o schema do item',
    group: 'Paleta de comandos',
    context: 'palette',
    defaults: ['Tab'],
    allowWhileTyping: true,
  },
  'palette.unpinSchema': {
    label: 'Soltar o schema fixado',
    group: 'Paleta de comandos',
    context: 'palette',
    defaults: ['Shift+Tab'],
    allowWhileTyping: true,
  },

  // ── Editor SQL ──────────────────────────────────────────────────────────
  'editor.run': {
    label: 'Executar a consulta',
    group: 'Editor SQL',
    context: 'editor',
    defaults: ['Mod+Enter', 'Mod+Shift+Enter'],
    allowWhileTyping: true,
  },
  // No macOS o Cmd+Space é do Spotlight. O Ctrl+Space / Alt+i da própria
  // lib seguem valendo como alternativa.
  'editor.complete': {
    label: 'Mostrar sugestões',
    group: 'Editor SQL',
    context: 'editor',
    defaults: { mac: ['Shift+Space'], other: ['Mod+Space'] },
    allowWhileTyping: true,
  },
} satisfies Record<string, ShortcutAction>;

export type ShortcutId = keyof typeof SHORTCUT_ACTIONS;

export const SHORTCUT_IDS = Object.keys(SHORTCUT_ACTIONS) as ShortcutId[];

export const shortcutAction = (id: ShortcutId): ShortcutAction =>
  SHORTCUT_ACTIONS[id];

export const isShortcutId = (id: string): id is ShortcutId =>
  Object.prototype.hasOwnProperty.call(SHORTCUT_ACTIONS, id);

/** A ordem dos grupos na tela de configuração */
export const SHORTCUT_GROUPS: ShortcutGroup[] = [
  'Geral',
  'Abas',
  'Assistente',
  'Árvore',
  'Aba de tabela',
  'Grade',
  'Paleta de comandos',
  'Editor SQL',
];

/**
 * Dois contextos podem receber a mesma tecla ao mesmo tempo? O global vale em
 * todo lugar; a aba de tabela inclui a grade e os editores dela (o filtro).
 */
export function contextsOverlap(a: ShortcutContext, b: ShortcutContext) {
  if (a === b || a === 'global' || b === 'global') return true;
  const pair = (x: ShortcutContext, y: ShortcutContext) =>
    (a === x && b === y) || (a === y && b === x);
  return pair('table', 'grid') || pair('table', 'editor');
}
