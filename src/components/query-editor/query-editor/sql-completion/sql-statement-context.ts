import { syntaxTree } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';
import type {
  SqlClause,
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

/**
 * Keywords que definem a cláusula. `insert`/`delete` caem em `start` de propósito: o
 * keyword que importa (`into`, `from`) vem logo em seguida e sobrescreve.
 */
const CLAUSE_KEYWORDS = new Map<string, SqlClause>([
  ['select', 'select'],
  ['from', 'from'],
  ['join', 'join'],
  ['on', 'on'],
  ['using', 'on'],
  ['where', 'where'],
  ['group', 'group'],
  ['having', 'having'],
  ['order', 'order'],
  ['limit', 'limit'],
  ['offset', 'limit'],
  ['fetch', 'limit'],
  ['set', 'set'],
  ['update', 'update'],
  ['into', 'into'],
  ['values', 'values'],
  ['returning', 'returning'],
  ['insert', 'start'],
  ['delete', 'start'],
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

/**
 * Statement sob o cursor mais a posição usada para achá-lo. Quem sonda a árvore depois
 * (a detecção de `Parens`) precisa da posição **ajustada**, não da posição crua.
 */
function enclosingStatement(
  state: EditorState,
  pos: number,
): { statement: SyntaxNode; scanPos: number } | null {
  // Espaço em branco no fim do documento fica fora do nó `Statement`, então `where |`
  // não acharia statement nenhum. Recuamos até o último caractere significativo.
  let scanPos = pos;

  while (
    scanPos > 0 &&
    /\s/.test(state.doc.sliceString(scanPos - 1, scanPos))
  ) {
    scanPos -= 1;
  }

  // Depois de `;` começa um statement novo: não herdar as tabelas do anterior.
  if (scanPos > 0 && state.doc.sliceString(scanPos - 1, scanPos) === ';') {
    return null;
  }

  let node: SyntaxNode | null = syntaxTree(state).resolveInner(scanPos, -1);

  while (node && node.name !== 'Statement') {
    node = node.parent;
  }

  return node ? { statement: node, scanPos } : null;
}

/** `true` se houver um nó `Parens` entre a posição e o statement. */
function hasParensAncestor(
  state: EditorState,
  scanPos: number,
  statement: SyntaxNode,
): boolean {
  let node: SyntaxNode | null = syntaxTree(state).resolveInner(scanPos, -1);

  while (node && node !== statement && node.name !== 'Statement') {
    if (node.name === 'Parens') return true;
    node = node.parent;
  }

  return false;
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
  const enclosing = enclosingStatement(state, pos);

  if (!enclosing) {
    return { tables: [], atTopLevel, clause: 'start' };
  }

  const { statement, scanPos } = enclosing;

  const tables: StatementTableRef[] = [];
  const seen = new Set<string>();

  let expect: 'none' | 'table' | 'alias' = 'none';
  let inTableList = false;
  let current: StatementTableRef | null = null;
  let clause: SqlClause = 'start';

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

      // Só a última cláusula **antes** do cursor conta. Keyword não reconhecido não
      // altera nada — é o que mantém `GROUP BY |` em `group` e o que salva os nomes de
      // coluna que o dialeto também trata como palavra reservada (`id`, `name`, `count`).
      if (scan.from < pos) {
        const mapped = CLAUSE_KEYWORDS.get(keyword);

        if (mapped) clause = mapped;
      }

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

  // `INSERT INTO t (|` continua em `into`, mas o cursor está dentro dos parênteses da
  // lista de colunas. O gate por `into` é essencial: sem ele `count(|`, `(a + |` e
  // `USING (|` também virariam posição de coluna.
  if (clause === 'into' && hasParensAncestor(state, scanPos, statement)) {
    clause = 'insert-columns';
  }

  return { tables, atTopLevel, clause };
}
