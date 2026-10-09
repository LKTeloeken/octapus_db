# Backend Rust — Guia de Arquitetura e Integração com o Front

Documento de referência do backend em Rust/Tauri do **octapus_db**. Explica como o
back está organizado, todos os comandos (`invoke`) disponíveis, os formatos de
dados, e instruções passo a passo para implementar cada tela do front.

> **Bancos suportados:** PostgreSQL, MongoDB, Redis e SQLite. Os quatro expõem
> **os mesmos comandos** — o front não precisa saber o tipo do banco para a maioria
> das operações. Onde o comportamento muda, o comando `get_capabilities` informa
> o que renderizar.
>
> **Estrutura do banco:** Postgres, Mongo e SQLite respondem pelo **catálogo de
> metadados** (comandos `catalog_*`, §1.1 e §6.3) — o front pede fatias (filhos de um
> nó, busca, resolução), nunca a estrutura inteira. Só o Redis ainda usa
> `list_schemas_with_tables`.

---

## 1. Arquitetura em camadas

```
Front (invoke) ──▶ commands/   handlers #[tauri::command], validam e delegam
                   services/    orquestração: ConnectionService, CatalogService, ...
                   catalog/     núcleo do catálogo de metadados (Rust puro, sem banco)
                   adapters/    DatabaseAdapter trait → postgres | mongo | redisdb | sqlite
                                + fontes do catálogo (`introspect.rs` de cada banco)
                   storage/     SQLite local (servers, sessão) + vault (senhas criptografadas)
Front (listen) ◀── evento `catalog-event` (progresso dos catálogos)
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
2. `connect_adapter` (`commands/mod.rs`) devolve o adapter do cache — com um ping só se
   ele ficou parado mais de 30 s. No cache-miss busca o `Server` no SQLite, decifra a
   senha pelo cofre (`vault`) e cria o pool.
3. O adapter executa e devolve um `QueryResult` já serializado.

Custo fixo por comando: o pool usa `RecyclingMethod::Fast` com um hook que só valida
conexões paradas, e os metadados usam `prepare_cached` — um comando de metadado sobre
uma conexão em uso custa **uma** ida e volta ao banco.

### 1.1 Catálogo de metadados

A estrutura dos bancos (schemas, tabelas, coleções) mora no backend, num catálogo por
`(serverId, database)`. O front só pede fatias. Motivo e medições:
[perf/catalog/](perf/catalog/README.md) — num banco com 5.000 schemas × 150 tabelas, a
listagem inteira (com o tamanho de cada tabela) levava o backend ao OOM.

- **`catalog/`** — o núcleo, sem banco nem rede: nomes internados, **formatos**
  (conjuntos de relações compartilhados entre schemas iguais), índice nome → formatos,
  `children` paginado/filtrado, busca fuzzy agrupada (`nucleo-matcher`), resolução de
  nome sem schema pelo `search_path`, drift entre tenants, agrupamento por formato e
  persistência em disco (`OCTCAT` + versão + lz4/postcard cifrado com a chave do vault;
  versão diferente = arquivo descartado e refeito). Escopo salvo (`NameScope`) também
  mora aqui.
- **Fontes** (trait `CatalogSource`, `catalog/source.rs`), uma por banco, numa conexão
  **dedicada** (não disputa com o pool do editor):
  - **Postgres** (`adapters/postgres/introspect.rs`): transação `REPEATABLE READ READ
    ONLY` com `SET LOCAL statement_timeout/lock_timeout` e `jit = off`. Camada 0 = nomes
    dos schemas; depois uma varredura calcula formato + fingerprint (`sum(hashtext(oid:xmin))`)
    de cada schema e só **um representante por formato** traz as relações. Detecta
    capacidade por versão/fork e cai num caminho em massa quando não dá.
  - **Mongo** (`adapters/mongo/introspect.rs`): um `listCollections` com `nameOnly` e
    `authorizedCollections`; tamanho só da coleção aberta (`$collStats`).
  - **SQLite** (`adapters/sqlite/introspect.rs`): `sqlite_schema`; fingerprint pelo
    `schema_version`; tamanho pelo `dbstat`.
  - Mongo e SQLite não têm schema: as relações ficam num schema **sem nome**
    (`FLAT_SCHEMA = ""`).
- **`CatalogService`** (`services/catalog.rs`):
  - **duas faixas** por catálogo, cada uma com sua conexão: sincronização (segundo plano)
    e prioridade (o usuário está esperando: abrir um schema ainda não carregado, o
    tamanho da tabela aberta). Pedidos urgentes simultâneos viram um lote;
  - **revalida por evento** — abrir o database (se a última sincronização tem mais de
    60 s) ou refresh manual — nunca por polling;
  - abre do **disco** na hora e revalida em seguida; salva a cada sincronização;
  - **manutenção a cada minuto**: salva o pendente, fecha conexões paradas há 5 min,
    tira da memória os catálogos menos usados acima de 256 MB (ficam no disco) e, na
    partida, apaga arquivos sem uso há 30 dias;
  - servidor editado ou excluído → `forget_server` tira da memória e do disco.
  - A senha só é decifrada ao criar a entrada do catálogo (cache-miss).
- **Eventos** `catalog-event` (`models/catalog.rs`): `syncing`, `schemas` (camada 0),
  `relations`, `ready` (com `added`/`removed`/`changed`), `error`, `cancelled`.

**Para mexer:** consulta nova → método em `Catalog` (`catalog/query.rs`) + comando em
`commands/catalog.rs`; banco novo → implementar `CatalogSource` e incluir em
`adapters::has_catalog`/`create_catalog_source`. Nada de voltar a listar a estrutura
inteira (`list_schemas_with_tables` é default `UnsupportedType` na trait, só o Redis
implementa) nem de pedir `pg_total_relation_size`/`$collStats` em massa.

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
// (mysql ainda não tem adapter — retorna erro "coming soon")

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
  connectionUri: string | null;   // URI completa (Atlas, Redis cloud); no SQLite, o caminho do arquivo
  createdAt: number;              // epoch em segundos
  // Escopo salvo: padrões separados por vírgula; `*` e `?`; `!` na frente exclui.
  // null = sem restrição. Ver "Escopo" em §4.
  scopeDatabases: string | null;  // databases visíveis (list_databases)
  scopeSchemas: string | null;    // schemas visíveis (Postgres: catálogo e list_schemas)
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
  scopeDatabases?: string | null; // só espaços = null
  scopeSchemas?: string | null;
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
  whereExpr?: string;       // Postgres: expressão WHERE (sem a keyword); Mongo/Redis rejeitam se preenchido
  sort?: SortSpec[];
  limit?: number;           // default 500
  offset?: number;          // default 0
  countTotal?: boolean;     // default false
  unlimited?: boolean;      // ignora o limit e traz tudo (exportação); default false
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

// Árvore completa em uma chamada — só o Redis (os demais usam o catálogo):
interface DatabaseStructure {
  schemas: { name: string; tables: { name: string; tableType: string }[] }[];
  fetchedAt: number; // epoch em ms
}
```

### Catálogo de metadados
Espelho em `src/api/types/catalog.types.ts` (Rust: `models/catalog.rs` e
`catalog/model.rs`).
```ts
type CatalogNodeKind =
  'schema' | 'table' | 'view' | 'materializedView' | 'foreign' | 'partitioned';

interface CatalogNode {
  name: string;
  kind: CatalogNodeKind;
  childCount: number | null;     // tabelas de um schema, partições de um pai; null = não carregado
  state: 'unloaded' | 'loaded' | 'stale' | null;   // só schemas
  drift: { missing: number; extra: number } | null; // só schemas: diferença p/ o formato dominante
}

// Nó cujos filhos se pede. Mongo/SQLite: { kind: 'schema', schema: '' }
type CatalogPath =
  | { kind: 'schemas' }
  | { kind: 'schema'; schema: string }
  | { kind: 'partitions'; schema: string; table: string }
  | { kind: 'shape'; key: string };      // schemas de um grupo (árvore agrupada)

interface CatalogPage<T> { total: number; offset: number; items: T[] } // total já filtrado

interface SchemaGroup { total: number; sample: string[] }

// Mesma relação em vários schemas = um resultado (`schemas`); busca `schema.tabela`
// devolve pares (`schema`); um schema não tem nenhum dos dois.
interface CatalogSearchHit {
  serverId: number; database: string;
  name: string; kind: CatalogNodeKind; score: number;
  schema: string | null; schemas: SchemaGroup | null;
}

type CatalogResolution =
  | { status: 'found'; schema: string; table: string; kind: CatalogNodeKind }
  | { status: 'ambiguous'; table: string; schemas: SchemaGroup }
  | { status: 'notFound' };

interface CatalogStatus {
  serverId: number; database: string;
  syncing: boolean;
  fetchedAt: number | null;      // última revalidação concluída (ms)
  fromDisk: boolean;             // veio do arquivo e ainda não revalidou
  error: string | null;
  serverVersion: string | null;
  stats: { schemas; loaded; stale; unloaded; shapes; distinctNames; relations; approxHeapBytes };
}

// Árvore agrupada por formato
interface ShapeGroup {
  key: string;                   // hash do conteúdo do formato; 'other' para o resto
  role: 'dominant' | 'variant' | 'other';
  tables: number | null;
  missing: string[]; extra: string[];   // só nas variações
  schemas: number;
}

interface CatalogDriftReport {
  dominant: { tables: number; schemas: SchemaGroup } | null;
  divergent: { schemas: SchemaGroup; missing: string[]; extra: string[] }[];
}

// "Copiar diagnóstico": só contagens e tempos, nenhum nome
interface CatalogDiagnostics {
  appVersion: string;
  status: CatalogStatus;
  lastSync: { at; strategy: 'shapeFirst' | 'bulk'; schemas; shapes; fetched; shared;
              added; removed; changed; layer0Ms; totalMs } | null;
  drift: { dominantTables: number | null; dominantSchemas; divergentGroups; divergentSchemas };
}

// Evento `catalog-event`
type CatalogEvent = { serverId: number; database: string } & (
  | { type: 'syncing' }
  | { type: 'schemas'; added: string[]; removed: string[] }
  | { type: 'relations'; schemas: number }
  | { type: 'ready'; added: string[]; removed: string[]; changed: string[]; fetchedAt: number | null }
  | { type: 'error'; message: string }
  | { type: 'cancelled' }
);
```

### AdapterCapabilities
```ts
interface AdapterCapabilities {
  hasSchemas: boolean;          // só Postgres true
  hasPrimaryKeys: boolean;      // Postgres/Mongo/SQLite true; Redis false
  supportsSql: boolean;         // Postgres e SQLite
  supportsTransactions: boolean;// Postgres e SQLite
  supportsIndexes: boolean;     // Postgres/Mongo/SQLite true; Redis false
  browsable: boolean;           // os quatro true
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
| `update_server` | `{ id, input: ServerInput }` | `Server` (derruba conexões e esquece os catálogos do server) |
| `delete_server` | `{ id }` | `void` (apaga o segredo, as conexões e os catálogos) |

### Conexão

| Comando | Args | Retorno | Observação |
|---|---|---|---|
| `connect` | `{ serverId, database? }` | `boolean` | Testa a conexão; `database` opcional usa o default do tipo |
| `test_connection` | `{ serverId }` | `boolean` | Igual a `connect` mas sempre no database default |
| `disconnect` | `{ serverId, database? }` | `void` | Sem `database` → desconecta todos os databases do server |
| `get_pool_stats` | `{ serverId, database }` | `PoolStats \| null` | Só Postgres devolve stats |

`PoolStats`: `{ size, available, inUse, waiting }` (todos `number`).

### Estrutura

| Comando | Args | Retorno |
|---|---|---|
| `list_databases` | `{ serverId }` | `DatabaseInfo[]` |
| `list_schemas` | `{ serverId, database }` | `SchemaInfo[]` |
| `list_tables` | `{ serverId, database, schema }` | `TableInfo[]` |
| `list_columns` | `{ serverId, database, schema, table }` | `ColumnInfo[]` |
| `list_indexes` | `{ serverId, database, schema, table }` | `IndexInfo[]` |
| `list_schemas_with_tables` | `{ serverId, database }` | `DatabaseStructure` *(só Redis)* |

> `list_databases` devolve **só os nomes** em Postgres e Mongo (`sizeBytes: null` — o
> tamanho de cada database custava de segundos a minutos) e já aplica o escopo
> `scopeDatabases`; `list_schemas` aplica o `scopeSchemas`. `list_columns`/`list_indexes`
> continuam sob demanda ao abrir uma tabela. Para schemas e tabelas, use o catálogo.

### Catálogo de metadados (Postgres, Mongo, SQLite — §1.1)

Tudo responde **da memória do backend**; só carregar um schema que ainda não chegou ou
pedir um tamanho vai ao banco, pela faixa de prioridade. Listas têm teto de 1.000 itens
por chamada.

| Comando | Args | Retorno | Vai ao banco? |
|---|---|---|---|
| `catalog_open` | `{ serverId, database }` | `CatalogStatus` | abre (do disco, se houver) e revalida em 2º plano se a última for > 60 s |
| `catalog_status` | `{ serverId, database }` | `CatalogStatus \| null` | não (null = não está aberto) |
| `catalog_children` | `{ serverId, database, path, filter?, offset, limit }` | `CatalogPage<CatalogNode>` | só se o schema ainda não carregou |
| `catalog_search` | `{ query, limit, serverId?, database?, schema? }` | `CatalogSearchHit[]` | não — sem `serverId`/`database`, busca em todos os catálogos abertos; com `schema`, só as relações desse schema (nome exato; busca vazia lista todas) |
| `catalog_resolve` | `{ serverId, database, table, searchPath }` | `CatalogResolution` | não |
| `catalog_complete` | `{ serverId, database, schema?, prefix, limit }` | `CatalogNode[]` | só para carregar o schema |
| `catalog_shapes` | `{ serverId, database }` | `ShapeGroup[]` | não (vazio = sem formato repetido) |
| `catalog_drift` | `{ serverId, database }` | `CatalogDriftReport` | não |
| `catalog_refresh` | `{ serverId, database, schema? }` | `CatalogStatus` | sim: um schema na hora, ou o database em 2º plano |
| `catalog_cancel` | `{ serverId, database }` | `boolean` | interrompe a sincronização em andamento |
| `catalog_relation_size` | `{ serverId, database, schema, table }` | `number \| null` | sim (só da tabela aberta) |
| `catalog_diagnostics` | `{ serverId, database }` | `CatalogDiagnostics` | não |

O progresso chega pelo evento global **`catalog-event`** (`CatalogEvent`): o front escuta
com `listen` e invalida só as fatias daquele database. No Redis, os comandos que abrem
um catálogo respondem `UnsupportedDatabase`.

**Escopo salvo:** `scopeDatabases`/`scopeSchemas` do `Server` usam a sintaxe de
`catalog::NameScope` — padrões separados por vírgula ou quebra de linha, `*` qualquer
trecho, `?` um caractere, `!` na frente exclui, sem diferenciar maiúsculas; sem padrão
de inclusão, tudo entra (menos o excluído). Schema fora do escopo não é lido, listado nem
buscado. O escopo entra na identidade do arquivo do catálogo.

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

### Exportação

| Comando | Args | Retorno |
|---|---|---|
| `write_export_file` | `{ path, contents }` | `void` |

O front serializa (CSV/JSON/SQL) e escolhe o caminho pelo `plugin-dialog`; o backend
só grava. JSON e SQL também podem ir para a área de transferência
(`plugin-clipboard-manager`) quando o resultado é pequeno. Para exportar além da
página carregada, reenvie `fetch_table_data` (ou `execute_query`) com
`unlimited: true`.

### Sessão do workspace (abas abertas — SQLite local)

| Comando | Args | Retorno |
|---|---|---|
| `load_session` | — | `string \| null` |
| `save_session` | `{ snapshot }` | `void` |

Guarda as abas abertas para reabri-las depois de fechar/atualizar o app. O snapshot é
um JSON **opaco** para o backend — formato e versão são do front
(`src/stores/tabs-session.ts`); o backend só grava numa linha única da tabela
`workspace_session` do `app.db`. `save_session` só responde depois de gravar em disco.

### Preferências do app (SQLite local)

| Comando | Args | Retorno |
|---|---|---|
| `get_settings` | — | `AppSettings` |
| `update_settings` | `{ settings }` | `AppSettings` (o que ficou gravado) |

`AppSettings` (`models/settings.rs`) vira uma linha chave/valor (JSON) por campo na
tabela `app_settings` do `app.db`. Configuração nova = campo no struct, com o padrão no
`Default`, e o tipo espelho em `src/api/types/settings.types.ts` — sem migração: o que
nunca foi salvo (ou ficou ilegível) volta com o padrão. O tema **não** mora aqui: fica
no `ui-store` (localStorage), lido antes da primeira pintura.

### Auto-update — canal beta (só desktop)

| Comando | Args | Retorno |
|---|---|---|
| `check_beta_update` | — | metadados do `Update` do plugin, ou `null` |

O canal estável usa o `check()` do `@tauri-apps/plugin-updater` com o endpoint fixo do
`tauri.conf.json` (`releases/latest/download/latest.json` — o GitHub nunca marca uma
pre-release como "latest"). Com `betaUpdates` ligado, o front chama `check_beta_update`:
ele lista as releases na API do GitHub, pega a de **maior semver** (releases e
pre-releases publicadas, tag `app-v<versão>`, com `latest.json`) e roda o updater com
esse manifesto. A assinatura segue verificada pela chave pública da config, e o update
volta registrado na tabela de recursos do webview — o front monta um `Update` do plugin
e baixa pelo `downloadAndInstall` de sempre. O `release.yml` publica como pre-release
toda versão com sufixo (`1.1.0-beta.2`), e o `nix-release.yml` as ignora.

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
Fluxo com o catálogo (Postgres, Mongo, SQLite):
```ts
const dbs = await invoke<DatabaseInfo[]>('list_databases', { serverId });
// ao expandir um database: abre o catálogo (do disco na hora, revalida em 2º plano)
const status = await invoke<CatalogStatus>('catalog_open', { serverId, database });
// a primeira janela de schemas (filtro e "carregar mais" mudam filter/limit)
const schemas = await invoke<CatalogPage<CatalogNode>>('catalog_children', {
  serverId, database, path: { kind: 'schemas' }, filter: null, offset: 0, limit: 500,
});
// ao expandir um schema (Mongo/SQLite: schema: '' direto sob o database)
const tables = await invoke<CatalogPage<CatalogNode>>('catalog_children', {
  serverId, database, path: { kind: 'schema', schema }, filter: null, offset: 0, limit: 500,
});
// ao expandir uma tabela (colunas/índices):
const cols = await invoke<ColumnInfo[]>('list_columns', {
  serverId, database, schema, table,
});
// e escute o progresso:
await listen<CatalogEvent>('catalog-event', ({ payload }) => { /* invalida as fatias */ });
```
- **Logo depois de abrir** a lista pode vir vazia: a camada 0 chega pelo evento
  `schemas` em poucas centenas de ms. Um schema ainda não carregado vem com
  `childCount: null` e `state: 'unloaded'`; abrir ele carrega na hora.
- **Partições** ficam dentro da tabela particionada (`kind: 'partitioned'`): peça
  `{ kind: 'partitions', schema, table }`.
- **Agrupado por formato:** `catalog_shapes` dá os grupos e `{ kind: 'shape', key }` os
  schemas de cada um. O `drift` de cada schema diz quantas tabelas faltam/sobram em
  relação ao formato dominante.
- **Tamanho:** só da tabela aberta, com `catalog_relation_size`.
- **Redis** continua com `list_schemas_with_tables`: `database` = índice numérico
  (`"0"`...), `table` = grupo de keys pelo prefixo antes do primeiro `:` (ex.: `user`),
  colunas fixas `key / type / ttl / value`. Keys sem `:` ficam no grupo `(root)`.

**Adapte a UI pelo `capabilities`:** se `hasSchemas === false` (Mongo, SQLite, Redis),
não mostre o nível "schema" — pule direto database → tabelas/collections/grupos.
- **Mongo:** `database` = database, `table` = collection, colunas inferidas por
  amostragem de ~100 documentos (campo ausente vira `isNullable`).
- **SQLite:** um arquivo, database `main`; tabelas e views do `sqlite_schema`.

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
Ao clicar numa tabela, **não monte o SELECT no front** — chame `fetch_table_data`.
O back gera o SQL/find/scan, valida a expressão WHERE (Postgres) e pagina no servidor.

```ts
const request: TableDataRequest = {
  schema: 'public',          // null para Mongo/Redis
  table: 'users',
  whereExpr: "age >= 18 AND city IN ('SP', 'RJ')",
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
- **Filtrar (Postgres):** envie `whereExpr` com uma expressão SQL (sem `WHERE`).
  O back valida que é uma única expressão (`sqlparser`) e interpola
  `WHERE (expr)`. Vazio/omitido = sem filtro. Aplicar no Enter, não a cada tecla.
- **Paginar:** incremente `offset`; use `hasMore` para o botão "próxima".
- O retorno é o mesmo `QueryResult` do editor → **reaproveite o componente de
  grid**. `editableInfo` vem preenchido quando a tabela tem PK (Postgres) ou
  `_id` (Mongo); no Redis vem `null` (edite via comando nativo).

Comportamento por banco:
- **Postgres:** `SELECT ... WHERE (expr) ORDER BY ... LIMIT/OFFSET`. Sort
  continua parametrizado por identificador quoted; a expressão WHERE é literal.
- **Mongo:** `find().sort().skip().limit()` — `whereExpr` preenchido é rejeitado.
- **Redis:** `SCAN MATCH grupo:*`, ordena client-side sobre
  `key/type/ttl/value` (varredura limitada a 50k keys por sweep). `whereExpr`
  preenchido é rejeitado.

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
  catalogChildren: (serverId: number, database: string, path: CatalogPath, limit = 500) =>
    call<CatalogPage<CatalogNode>>('catalog_children', {
      serverId, database, path, filter: null, offset: 0, limit,
    }),
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

| Conceito | PostgreSQL | MongoDB | SQLite | Redis |
|---|---|---|---|---|
| `database` | database | database | `main` (o arquivo) | índice numérico (`"0"`) |
| `schema` | schema real | `""` no catálogo (`hasSchemas=false`) | `""` no catálogo | ignorado |
| `table` | tabela/view | collection | tabela/view | grupo de keys por prefixo `:` |
| estrutura | catálogo | catálogo | catálogo | `list_schemas_with_tables` |
| `columns` | colunas reais | inferidas por amostragem | colunas reais | `key/type/ttl/value` |
| PK / edição | PK real | `_id` | PK real | sem edição inline |
| editor livre | SQL | `db.coll.find({...})` | SQL | `GET`, `HGETALL`, `SCAN`... |
| `get_capabilities` | tudo `true` | sem schema/SQL/transação | sem schema | só `browsable` |

O front pode ser **uniforme**: use os mesmos componentes para os quatro bancos e
deixe `get_capabilities` decidir o que esconder (nível schema, editor SQL,
edição inline). Os comandos e os formatos de retorno são idênticos.
