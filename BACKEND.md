# Backend Rust — Guia de Arquitetura e Integração com o Front

Documento de referência do backend em Rust/Tauri do **octapus_db**. Explica como o
back está organizado, todos os comandos (`invoke`) disponíveis, os formatos de
dados, e instruções passo a passo para implementar cada tela do front.

> **Bancos suportados:** PostgreSQL, MongoDB e Redis. Os três expõem **exatamente
> os mesmos comandos** — o front não precisa saber o tipo do banco para a maioria
> das operações. Onde o comportamento muda, o comando `get_capabilities` informa
> o que renderizar.

---

## 1. Arquitetura em camadas

```
Front (invoke) ──▶ commands/   handlers #[tauri::command], validam e delegam
                   services/    wrappers finos de orquestração
                   adapters/    DatabaseAdapter trait → postgres | mongo | redisdb
                   storage/     SQLite local (servers) + vault (senhas criptografadas)
```

- **`adapters/`** — toda conexão e execução de query vive aqui. Cada banco
  implementa a trait `DatabaseAdapter` (`adapters/traits.rs`). A factory
  `create_adapter` (`adapters/mod.rs`) escolhe a implementação pelo `dbType`.
- **`ConnectionService`** (`services/connection.rs`) — cache de adapters por
  `(serverId, database)`. A conexão é **lazy**: criada na primeira operação e
  reaproveitada (pool interno). Não existe um "abrir conexão" explícito — chamar
  qualquer comando com `serverId`+`database` já conecta sob demanda.
- **`storage/`** — os servidores cadastrados ficam num SQLite local (`app.db`).
  As **senhas são criptografadas por um cofre próprio** (`storage/vault.rs`,
  AES-256-GCM com chave presa ao dispositivo em `vault.key`); só o ciphertext fica
  na coluna do SQLite. Senhas de versões antigas (no keychain do SO) são migradas
  para o cofre na primeira conexão.

### Como uma chamada flui
1. Front faz `invoke('execute_query', { serverId, database, query })`.
2. O handler busca o `Server` no SQLite e decifra a senha pelo cofre (`vault`).
3. `get_or_connect(server, database)` devolve o adapter (do cache ou novo).
4. O adapter executa e devolve um `QueryResult` já serializado.

---

## 2. Convenções de chamada (Tauri v2)

- **Nome do comando:** `snake_case` (ex.: `fetch_table_data`).
- **Argumentos:** objeto com chaves em **`camelCase`** — o Tauri converte
  automaticamente do Rust `server_id` para o JS `serverId`.
- **Retorno:** Promise. Sucesso → o objeto descrito em cada comando. Erro → a
  Promise **rejeita com uma `string`** (mensagem legível, ex.:
  `"Connection error: ..."`). Sempre use `try/catch`.

```ts
import { invoke } from '@tauri-apps/api/core';

try {
  const result = await invoke<QueryResult>('execute_query', {
    serverId: 1,
    database: 'postgres',
    query: 'SELECT * FROM users',
  });
} catch (err) {
  // err é uma string com a mensagem de erro
  console.error(err);
}
```

Sugestão: criar um wrapper tipado por comando (ver §7).

---

## 3. Modelos de dados (formato JSON recebido pelo front)

Todos os campos chegam em `camelCase`.

### Server / ServerInput
```ts
type DatabaseType = 'postgres' | 'mongodb' | 'redis' | 'mysql' | 'sqlite';
// (mysql e sqlite ainda não têm adapter — retornam erro "coming soon")

interface Server {
  id: number;
  name: string;
  dbType: DatabaseType;
  host: string;
  port: number;
  username: string;
  // password NUNCA é serializada para o front
  defaultDatabase: string | null;
  sslEnabled: boolean;
  connectionUri: string | null;   // URI completa (Atlas, Redis cloud) — opcional
  createdAt: number;              // epoch em segundos
}

// Enviado em create_server / update_server:
interface ServerInput {
  name: string;
  dbType: DatabaseType;
  host: string;
  port: number;
  username: string;
  password: string;               // criptografada no cofre do back
  defaultDatabase?: string | null;
  sslEnabled?: boolean | null;
  connectionUri?: string | null;
}
```

### QueryResult (retorno de query e de browse)
```ts
interface QueryResult {
  columns: QueryColumnInfo[];
  rows: (string | null)[][];      // matriz; cada célula é string ou null
  rowCount: number;               // nº de linhas NESTA página
  totalCount: number | null;      // total sem paginação (só se countTotal=true)
  hasMore: boolean;               // existe próxima página?
  executionTimeMs: number;
  editableInfo: EditableInfo | null; // != null → linhas podem ser editadas
}

interface QueryColumnInfo {
  name: string;
  typeName: string;               // 'int4', 'text', 'string', 'long', 'hash'...
  typeOid: number | null;         // só Postgres
}
```
> **Importante:** todo valor de célula é **string ou `null`**. Conversões
> (número, data, boolean) são responsabilidade do front se precisar formatar.

### QueryOptions (paginação do editor livre)
```ts
interface QueryOptions {
  limit?: number;       // default 500
  offset?: number;      // default 0
  countTotal?: boolean; // default false — calcula totalCount
  unlimited?: boolean;  // default false — ignora limit/offset
}
```

### EditableInfo / RowEdit (edição inline)
```ts
interface EditableInfo {
  schema: string;
  table: string;
  primaryKeyColumns: string[];        // Postgres: PK real; Mongo: ['_id']
  primaryKeyColumnIndices: number[];  // posição das PKs em columns/rows
}

interface RowEdit {
  pkValues: (string | null)[];                  // na ordem de primaryKeyColumns
  changes: [string, string | null][];           // [coluna, novoValor]
}

interface RowInsert {
  // só as colunas preenchidas; as omitidas usam o default do banco (serial,
  // DEFAULT, _id automático)
  values: [string, string | null][];           // [coluna, valor]
}
```

### TableDataRequest (o coração do browse — §6.5)
```ts
interface TableDataRequest {
  schema?: string | null;   // Postgres: schema (default 'public'); Mongo/Redis: ignorado
  table: string;            // tabela / collection / grupo de keys
  filters?: ColumnFilter[];
  sort?: SortSpec[];
  limit?: number;           // default 500
  offset?: number;          // default 0
  countTotal?: boolean;     // default false
}

interface ColumnFilter {
  column: string;
  op: 'eq' | 'ne' | 'in' | 'like' | 'gt' | 'gte' | 'lt' | 'lte' | 'is_null' | 'not_null';
  values: string[];   // 'in' usa todos; comparações usam o primeiro; is_null/not_null ignora
}

interface SortSpec {
  column: string;
  direction: 'asc' | 'desc';   // default 'asc'
}
```

### Estrutura do banco
```ts
interface DatabaseInfo { name: string; sizeBytes: number | null; }
interface SchemaInfo   { name: string; tableCount: number | null; }

interface TableInfo {
  name: string;
  schema: string;
  tableType: 'table' | 'view' | 'materializedview' | 'foreign';
  rowEstimate: number | null;
}

interface ColumnInfo {
  name: string;
  ordinal: number;
  dataType: string;
  isNullable: boolean;
  defaultValue: string | null;
  isPrimaryKey: boolean;
  isForeignKey: boolean;
}

interface IndexInfo {
  name: string;
  columns: string[];
  isUnique: boolean;
  isPrimary: boolean;
  indexType: string;
}

// Árvore completa em uma chamada (para montar a sidebar):
interface DatabaseStructure {
  schemas: { name: string; tables: { name: string; tableType: string }[] }[];
  fetchedAt: number; // epoch em ms
}
```

### AdapterCapabilities
```ts
interface AdapterCapabilities {
  hasSchemas: boolean;          // Postgres true; Mongo/Redis false
  hasPrimaryKeys: boolean;      // Postgres/Mongo true; Redis false
  supportsSql: boolean;         // só Postgres
  supportsTransactions: boolean;// só Postgres
  supportsIndexes: boolean;     // Postgres/Mongo true; Redis false
  browsable: boolean;           // os três true
}
```

---

## 4. Referência completa de comandos

### Servidores cadastrados (CRUD — SQLite local, síncrono)

| Comando | Args | Retorno |
|---|---|---|
| `get_all_servers` | — | `Server[]` |
| `get_server` | `{ id }` | `Server` |
| `create_server` | `{ input: ServerInput }` | `Server` |
| `update_server` | `{ id, input: ServerInput }` | `Server` (derruba conexões do server) |
| `delete_server` | `{ id }` | `void` (apaga o segredo + conexões) |

### Conexão

| Comando | Args | Retorno | Observação |
|---|---|---|---|
| `connect` | `{ serverId, database? }` | `boolean` | Testa a conexão; `database` opcional usa o default do tipo |
| `test_connection` | `{ serverId }` | `boolean` | Igual a `connect` mas sempre no database default |
| `disconnect` | `{ serverId, database? }` | `void` | Sem `database` → desconecta todos os databases do server |
| `get_pool_stats` | `{ serverId, database }` | `PoolStats \| null` | Só Postgres devolve stats |

`PoolStats`: `{ size, available, inUse, waiting }` (todos `number`).

### Estrutura (lazy loading — monte a sidebar incrementalmente)

| Comando | Args | Retorno |
|---|---|---|
| `list_databases` | `{ serverId }` | `DatabaseInfo[]` |
| `list_schemas` | `{ serverId, database }` | `SchemaInfo[]` |
| `list_tables` | `{ serverId, database, schema }` | `TableInfo[]` |
| `list_columns` | `{ serverId, database, schema, table }` | `ColumnInfo[]` |
| `list_indexes` | `{ serverId, database, schema, table }` | `IndexInfo[]` |
| `list_schemas_with_tables` | `{ serverId, database }` | `DatabaseStructure` |

> `list_schemas_with_tables` traz schemas+tabelas de uma vez (bom para a árvore
> inicial). `list_columns`/`list_indexes` continuam sob demanda ao expandir.

### Editor livre de queries

| Comando | Args | Retorno |
|---|---|---|
| `execute_query` | `{ serverId, database, query, options? }` | `QueryResult` |
| `execute_statement` | `{ serverId, database, statement }` | `StatementResult` |
| `execute_transaction` | `{ serverId, database, statements: string[] }` | `StatementResult[]` |
| `apply_row_edits` | `{ serverId, database, editable, edits }` | `StatementResult` |
| `insert_rows` | `{ serverId, database, editable, rows: RowInsert[] }` | `StatementResult` |
| `delete_rows` | `{ serverId, database, editable, pkValues: (string\|null)[][] }` | `StatementResult` |
| `cancel_query` | `{ serverId, database, queryId }` | `void` *(só Postgres; Mongo/Redis retornam "não suportado")* |

`StatementResult`: `{ affectedRows: number, executionTimeMs: number }`.

### Browse de tabela (paginado/ordenado/filtrado — sem digitar query)

| Comando | Args | Retorno |
|---|---|---|
| `fetch_table_data` | `{ serverId, database, request: TableDataRequest }` | `QueryResult` |
| `get_capabilities` | `{ serverId }` | `AdapterCapabilities` |

---

## 5. Sintaxe do editor livre por banco

`execute_query` / `execute_statement` recebem a **sintaxe nativa de cada banco**:

- **PostgreSQL:** SQL normal. `SELECT` é paginado automaticamente
  (`limit`/`offset` das `options`); demais statements vão direto.
- **MongoDB:** comandos estilo shell, argumentos em JSON5 (chaves sem aspas, aspas
  simples e vírgula final são aceitas):
  - `db.users.find({ age: { $gt: 18 } }, { name: 1 })`
  - `db.orders.aggregate([{ $match: { total: { $gte: 10 } } }])`
  - `db.users.countDocuments({})`, `db.users.distinct('city')`
  - escrita: `insertOne`, `insertMany`, `updateOne`, `updateMany`, `deleteOne`,
    `deleteMany`, `drop`
- **Redis:** um comando nativo por chamada:
  - `GET user:1`, `HGETALL session:abc`, `LRANGE fila 0 -1`, `SCAN 0 MATCH user:*`
  - escrita (`SET`, `DEL`, `EXPIRE`...) via `execute_statement` — `affectedRows`
    reflete o reply inteiro quando é numérico.

> Use `capabilities.supportsSql` para decidir o highlight/placeholder do editor.

---

## 6. Como implementar cada tela do front

### 6.1 Listar bancos cadastrados
```ts
const servers = await invoke<Server[]>('get_all_servers');
```
Renderize a lista. Para o formulário de novo banco, pré-preencha a porta pelo
tipo (Postgres 5432, Mongo 27017, Redis 6379) e mande `create_server` com o
`ServerInput`. A senha digitada é criptografada no cofre do back — em telas
seguintes o `Server` **nunca** traz a senha de volta (campo some no JSON).

### 6.2 Conectar
Não há "abrir conexão" pesado: a conexão é lazy. Para um botão de "testar":
```ts
const ok = await invoke<boolean>('connect', { serverId, database: null });
```
`database: null` conecta no database default do tipo (Postgres `postgres`,
Mongo `admin`, Redis `0`). Trate a rejeição (string) como falha de conexão.

### 6.3 Buscar a estrutura (sidebar em árvore)
Fluxo recomendado, lazy:
```ts
const dbs = await invoke<DatabaseInfo[]>('list_databases', { serverId });
// ao expandir um database:
const struct = await invoke<DatabaseStructure>('list_schemas_with_tables', {
  serverId, database,
});
// ao expandir uma tabela (colunas/índices):
const cols = await invoke<ColumnInfo[]>('list_columns', {
  serverId, database, schema, table,
});
```
**Adapte a UI pelo `capabilities`:** se `hasSchemas === false` (Mongo/Redis),
não mostre o nível "schema" — pule direto database → tabelas/collections/grupos.
- **Mongo:** `database` = database, `table` = collection, colunas inferidas por
  amostragem de ~100 documentos (campo ausente vira `isNullable`).
- **Redis:** `database` = índice numérico (`"0"`...), `table` = grupo de keys
  pelo prefixo antes do primeiro `:` (ex.: `user`), colunas fixas
  `key / type / ttl / value`. Keys sem `:` ficam no grupo `(root)`.

### 6.4 Janela de query (editor livre)
```ts
const result = await invoke<QueryResult>('execute_query', {
  serverId, database,
  query: editorText,
  options: { limit: 500, offset: page * 500, countTotal: true },
});
```
Renderize `result.columns` + `result.rows`. Use `hasMore`/`totalCount` para
paginação. Se `result.editableInfo != null`, habilite edição inline (§6.6).

### 6.5 Abrir aba com os dados de uma tabela (browse) — o pedido central
Ao clicar numa tabela, **não monte query no front** — chame `fetch_table_data`.
O back gera o SQL/find/scan, valida colunas (sem injection) e pagina no servidor.

```ts
const request: TableDataRequest = {
  schema: 'public',          // null para Mongo/Redis
  table: 'users',
  filters: [
    { column: 'age', op: 'gte', values: ['18'] },
    { column: 'city', op: 'in', values: ['SP', 'RJ'] },
  ],
  sort: [{ column: 'created_at', direction: 'desc' }],
  limit: 100,
  offset: 0,
  countTotal: true,
};

const data = await invoke<QueryResult>('fetch_table_data', {
  serverId, database, request,
});
```
- **Ordenar por uma coluna:** ao clicar no header, reenvie com
  `sort: [{ column, direction }]`.
- **Filtrar por uma ou mais colunas:** acumule `ColumnFilter` (são combinados com
  AND). `values` é sempre `string[]` mesmo para número/data — o back faz o cast.
- **Paginar:** incremente `offset`; use `hasMore` para o botão "próxima".
- O retorno é o mesmo `QueryResult` do editor → **reaproveite o componente de
  grid**. `editableInfo` vem preenchido quando a tabela tem PK (Postgres) ou
  `_id` (Mongo); no Redis vem `null` (edite via comando nativo).

Comportamento por banco (transparente para o front):
- **Postgres:** `SELECT ... WHERE ... ORDER BY ... LIMIT/OFFSET` parametrizado.
- **Mongo:** vira `find().sort().skip().limit()`; `eq`/`in` casam número **e**
  string; `like` usa `%`/`_` convertidos para regex.
- **Redis:** `SCAN MATCH grupo:*`, filtra/ordena client-side sobre
  `key/type/ttl/value` (varredura limitada a 50k keys por sweep).

### 6.6 Edição inline de células
Quando `editableInfo != null`, monte os `RowEdit` a partir das células alteradas:
```ts
const edits: RowEdit[] = [{
  pkValues: ['42'],                       // valores das PK na ordem de primaryKeyColumns
  changes: [['name', 'Novo Nome'], ['active', 'true']],
}];
await invoke<StatementResult>('apply_row_edits', {
  serverId, database, editable: data.editableInfo, edits,
});
```
Pegue os `pkValues` lendo `rows[i][editableInfo.primaryKeyColumnIndices[k]]`.

**Inserir e remover linhas** seguem o mesmo `editableInfo` (suportados em Postgres e
Mongo; no Redis `editableInfo` vem `null`). As alterações ficam pendentes no front e são
aplicadas em lote ao salvar:
```ts
// novas linhas (verde) — só as colunas preenchidas
await invoke<StatementResult>('insert_rows', {
  serverId, database, editable: data.editableInfo,
  rows: [{ values: [['name', 'Ana'], ['active', 'true']] }],
});
// remover linhas (vermelho) — tuplas de PK na ordem de primaryKeyColumns
await invoke<StatementResult>('delete_rows', {
  serverId, database, editable: data.editableInfo,
  pkValues: [['42'], ['43']],
});
```
Ordem recomendada ao salvar tudo junto: `delete_rows` → `insert_rows` → `apply_row_edits`,
seguido de refetch.

---

## 7. Camada de acesso sugerida no front

Centralize os `invoke` num módulo tipado — evita repetir nomes e facilita tratar
o erro-string em um lugar só:

```ts
// src/api/db.ts
import { invoke } from '@tauri-apps/api/core';

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw new Error(typeof e === 'string' ? e : 'Erro desconhecido no backend');
  }
}

export const db = {
  servers: () => call<Server[]>('get_all_servers'),
  createServer: (input: ServerInput) => call<Server>('create_server', { input }),
  connect: (serverId: number, database?: string) =>
    call<boolean>('connect', { serverId, database: database ?? null }),
  capabilities: (serverId: number) =>
    call<AdapterCapabilities>('get_capabilities', { serverId }),
  structure: (serverId: number, database: string) =>
    call<DatabaseStructure>('list_schemas_with_tables', { serverId, database }),
  tableData: (serverId: number, database: string, request: TableDataRequest) =>
    call<QueryResult>('fetch_table_data', { serverId, database, request }),
  query: (serverId: number, database: string, query: string, options?: QueryOptions) =>
    call<QueryResult>('execute_query', { serverId, database, query, options }),
  applyEdits: (serverId: number, database: string, editable: EditableInfo, edits: RowEdit[]) =>
    call<StatementResult>('apply_row_edits', { serverId, database, editable, edits }),
};
```

---

## 8. Resumo do mapeamento por banco

| Conceito | PostgreSQL | MongoDB | Redis |
|---|---|---|---|
| `database` | database | database | índice numérico (`"0"`) |
| `schema` | schema real | ignorado (`hasSchemas=false`) | ignorado |
| `table` | tabela/view | collection | grupo de keys por prefixo `:` |
| `columns` | colunas reais | inferidas por amostragem | `key/type/ttl/value` |
| PK / edição | PK real | `_id` | sem edição inline |
| editor livre | SQL | `db.coll.find({...})` | `GET`, `HGETALL`, `SCAN`... |
| `get_capabilities` | tudo `true` | sem schema/SQL/transação | só `browsable` |

O front pode ser **uniforme**: use os mesmos componentes para os três bancos e
deixe `get_capabilities` decidir o que esconder (nível schema, editor SQL,
edição inline). Os comandos e os formatos de retorno são idênticos.
