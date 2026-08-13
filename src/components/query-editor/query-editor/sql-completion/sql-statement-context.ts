import { syntaxTree } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';
import type {
  SqlStatementContext,
  StatementTableRef,
} from './sql-completion.types';

type SyntaxNode = ReturnType<ReturnType<typeof syntaxTree>['resolveInner']>;
type Doc = EditorState['doc'];

/** Keywords que abrem uma posição de tabela. */
const TABLE_INTRO = new Set(['from', 'join', 'update', 'into']);

/** Ruído entre o keyword e o nome da tabela (`left outer join x`). */
const JOIN_MODIFIERS = new Set([
  'left',
  'right',
  'inner',
  'outer',
  'full',
  'cross',
  'natural',
  'lateral',
  'only',
]);

/**
 * Keywords que encerram a lista de tabelas. A primeira metade é o `EndFrom` do lang-sql
 * (`dist/index.js`, l. 425); o resto cobre as cláusulas que ele não precisa tratar.
 */
const END_TABLE_LIST = new Set([
  'where',
  'group',
  'having',
  'order',
  'union',
  'intersect',
  'except',
  'all',
  'distinct',
  'limit',
  'offset',
  'fetch',
  'for',
  'set',
  'on',
  'using',
  'values',
  'returning',
  'select',
]);

/** Nós que o tokenizer usa para palavras reservadas do dialeto. */
const KEYWORD_NODES = new Set(['Keyword', 'Type', 'Builtin']);

const IDENTIFIER_NODES = new Set([
  'Identifier',
  'QuotedIdentifier',
  'CompositeIdentifier',
]);

/** Porta de `tokenBefore` do lang-sql: token anterior, pulando comentários. */
function tokenBefore(node: SyntaxNode): SyntaxNode {
  const cursor = node.cursor().moveTo(node.from, -1);

  while (/Comment/.test(cursor.name)) {
    cursor.moveTo(cursor.from, -1);
  }

  return cursor.node;
}

/** Porta de `idName` do lang-sql: texto do identificador, sem as aspas. */
function idName(doc: Doc, node: SyntaxNode): string {
  const text = doc.sliceString(node.from, node.to);
  const quoted = /^([`'"[])(.*)([`'"\]])$/.exec(text);

  return quoted ? quoted[2] : text;
}

function isPlainId(node: SyntaxNode | null): boolean {
  return (
    node != null &&
    (node.name === 'Identifier' || node.name === 'QuotedIdentifier')
  );
}

/** Porta de `pathFor` do lang-sql: `public.users` → `['public', 'users']`. */
function pathFor(doc: Doc, node: SyntaxNode): string[] {
  if (node.name !== 'CompositeIdentifier') {
    return [idName(doc, node)];
  }

  const path: string[] = [];

  for (let child = node.firstChild; child; child = child.nextSibling) {
    if (isPlainId(child)) {
      path.push(idName(doc, child));
    }
  }

  return path;
}

/**
 * `true` quando o cursor **não** está atrás de um ponto. Replica os ramos de
 * `sourceContext` do lang-sql (`dist/index.js`, l. 409).
 */
function isTopLevelPosition(state: EditorState, pos: number): boolean {
  const node = syntaxTree(state).resolveInner(pos, -1);

  if (
    node.name === 'Identifier' ||
    node.name === 'QuotedIdentifier' ||
    node.name === 'Keyword'
  ) {
    return tokenBefore(node).name !== '.';
  }

  return node.name !== '.';
}

function enclosingStatement(state: EditorState, pos: number): SyntaxNode | null {
  // Espaço em branco no fim do documento fica fora do nó `Statement`, então `where |`
  // não acharia statement nenhum. Recuamos até o último caractere significativo.
  let searchPos = pos;

  while (
    searchPos > 0 &&
    /\s/.test(state.doc.sliceString(searchPos - 1, searchPos))
  ) {
    searchPos -= 1;
  }

  // Depois de `;` começa um statement novo: não herdar as tabelas do anterior.
  if (searchPos > 0 && state.doc.sliceString(searchPos - 1, searchPos) === ';') {
    return null;
  }

  let node: SyntaxNode | null = syntaxTree(state).resolveInner(searchPos, -1);

  while (node && node.name !== 'Statement') {
    node = node.parent;
  }

  return node;
}

/**
 * Tabelas referenciadas no statement onde o cursor está, com os apelidos que o usuário
 * deu a elas. Subir até o nó `Statement` já isola statements separados por `;`.
 *
 * Igual ao `getAliases` do lang-sql, a varredura fica no nível de cima do statement e
 * **não desce em `Parens`** — subquery e CTE não entram (ver os limites no plano).
 */
export function getStatementContextAt(
  state: EditorState,
  pos: number,
): SqlStatementContext {
  const atTopLevel = isTopLevelPosition(state, pos);
  const statement = enclosingStatement(state, pos);

  if (!statement) {
    return { tables: [], atTopLevel };
  }

  const tables: StatementTableRef[] = [];
  const seen = new Set<string>();

  let expect: 'none' | 'table' | 'alias' = 'none';
  let inTableList = false;
  let current: StatementTableRef | null = null;

  const flush = () => {
    if (!current) return;

    const key = `${current.schemaHint ?? ''}.${current.table}`;

    if (!seen.has(key)) {
      seen.add(key);
      tables.push(current);
    }

    current = null;
  };

  for (let scan = statement.firstChild; scan; scan = scan.nextSibling) {
    const text = state.doc.sliceString(scan.from, scan.to);

    if (KEYWORD_NODES.has(scan.name)) {
      const keyword = text.toLowerCase();

      if (TABLE_INTRO.has(keyword)) {
        flush();
        expect = 'table';
        inTableList = true;
        continue;
      }

      // `users AS u`: o apelido é o próximo identificador.
      if (keyword === 'as') {
        continue;
      }

      if (expect === 'table' && JOIN_MODIFIERS.has(keyword)) {
        continue;
      }

      // Tabela com nome de palavra reservada (`user`, `order`, `session`): o tokenizer do
      // lezer só emite `Identifier` para palavras coladas num ponto, então aqui vem `Keyword`.
      if (expect === 'table' && !END_TABLE_LIST.has(keyword)) {
        flush();
        current = { table: text };
        expect = 'alias';
        continue;
      }

      flush();
      expect = 'none';
      inTableList = false;
      continue;
    }

    if (IDENTIFIER_NODES.has(scan.name)) {
      if (expect === 'table') {
        const path = pathFor(state.doc, scan);
        const table = path[path.length - 1];

        if (table) {
          flush();
          current = {
            table,
            schemaHint: path.length > 1 ? path[path.length - 2] : undefined,
          };
          expect = 'alias';
        }

        continue;
      }

      if (expect === 'alias' && current && !current.alias) {
        current.alias = idName(state.doc, scan);
        expect = 'none';
      }

      continue;
    }

    // `from users u, orders o` — a vírgula reabre a lista.
    if (scan.name === 'Punctuation' && text === ',' && inTableList) {
      flush();
      expect = 'table';
    }
  }

  flush();

  return { tables, atTopLevel };
}
