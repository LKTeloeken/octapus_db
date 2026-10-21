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
                   evento `backup-event` (andamento de backup/restauração)
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

### TLS das conexões
`adapters/tls.rs` monta o `rustls::ClientConfig` a partir do `SslMode` do servidor (nomes
e semântica do `sslmode` da libpq): `prefer`/`require` cifram sem verificar o certificado;
`verify-ca` exige cadeia até uma CA confiável; `verify-full` exige também o nome do host.
As CAs são as do sistema (no macOS, o Keychain — inclusive CA corporativa) ou só a do
`sslRootCert`. O Postgres usa os cinco modos e fixa o `sslmode` do tokio-postgres (o
padrão dele, `prefer`, caía para texto puro em silêncio). Mongo e Redis aceitam só
`disable`, `require` e `verify-full`; o que chega fora disso é ajustado ao gravar
(`SslMode::for_db_type`: `prefer` → `disable`, `verify-ca` → `verify-full`). Com
`connectionUri`, o Mongo e o Redis seguem o TLS da URI. Erros de handshake trazem a causa
(`error_chain`: "certificate not valid for name…", "UnknownIssuer"). E2E com CA própria:
`scripts/tls-e2e.sh up` e `cargo test --lib postgres::tls_e2e -- --ignored` (ver o
arquivo).

### Storage local e transporte (decisões medidas)
**Comandos síncronos no storage.** No Tauri, `#[tauri::command] fn` (sem `async`) roda na
thread principal. Os do storage (`get_all_servers`, `get_settings`, `load_session`…) foram
medidos num SQLite em arquivo com WAL, build de debug: todos abaixo de 0,4 ms por chamada
(`create_server` 0,33 · `get_all_servers` com 50 servidores 0,15 · `get_by_id` com
decifragem 0,04 · `save_session` de 200 KB 0,03). Ficam síncronos: movê-los para
`spawn_blocking` não muda nada perceptível. Reavaliar se surgir comando que segure o
`storage` por muito tempo (ex.: importar centenas de conexões numa chamada só).

**Transporte (túnel SSH, ainda não existe).** Host e porta são lidos em três pontos, todos
a partir do `Server`: `create_adapter` (pelo `ConnectionService`), `create_catalog_source`
(conexão dedicada do catálogo) e `backup::connection_args` (processo externo). O túnel
entra num serviço próprio (`services/transport.rs`) que, no cache-miss, devolve uma cópia
do `Server` apontando para `127.0.0.1:<porta local>` — os adapters continuam sem saber de
túnel. Cuidados já conhecidos:
- o túnel vive enquanto houver pool, conexão do catálogo ou job de backup usando-o
  (contagem de referências); `evict_idle` e `disconnect_server` precisam soltá-lo;
- o TLS `verify-full` confere o **host original**, não `127.0.0.1` — o nome do servidor
  para verificação tem de vir do `Server` cadastrado;
- `connection_identity` (catálogo) usa o host original, não o local;
- `mongodb+srv://` resolve hosts por DNS e não passa por um túnel simples.

### Tipos do front gerados (ts-rs)
Os modelos que cruzam o IPC levam `#[cfg_attr(test, derive(ts_rs::TS), ts(export))]` e
viram arquivos em `src/api/types/generated/` quando os testes rodam (`pnpm gen:types`, ou
qualquer `cargo test`); `.cargo/config.toml` aponta a pasta e faz `i64` virar `number`.
Só em teste: o binário não leva o ts-rs. O que o `serde` faz e o TS não enxerga se anota
no campo: `ts(skip)` (ex.: `Server.password`, que nunca sai), `ts(type = "...")`,
`ts(optional = nullable)` / `ts(as = "Option<T>", optional)` para campo de entrada que
pode faltar.

### Log do app
`logging.rs` liga o `tauri-plugin-log`: arquivo `octapus.log` na pasta de logs do SO
(`app_log_dir`; no macOS `~/Library/Logs/com.octapus-db.app/`), girando a cada 5 MB e
guardando 4 antigos, mais o terminal em dev. O app loga em `info` (`debug` em dev); as
dependências só a partir de `warn`. Um panic vai para o log antes do caminho padrão.
Entram: conexão criada/falha/reconexão por ping, sincronização do catálogo (início, fim
com contagens e tempos, cancelamento, falha, falha ao gravar em disco), jobs de backup e
restauração, importação de conexões e checagem do updater beta. Servidor pelo id e
database pelo nome — **nunca** senha, URI, texto de query nem nomes de schema/tabela.

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

- **Comparação entre dois schemas** (`catalog/columns.rs` + `CatalogSource::relation_columns`):
  as relações de cada schema vêm do catálogo (carregadas se preciso); só as colunas das
  tabelas em comum vão ao banco, numa consulta pelos pares (schema, tabela), com o próprio
  schema tirado de tipos e defaults (a sequência de cada tenant não é diferença). Fontes
  sem schema respondem `UnsupportedType`.

As fontes também respondem, só para as relações pedidas, `foreign_keys` e
`relation_details` (SQL) e `infer_shape` (Mongo: formato por `$sample`) — contexto do
assistente, ver §4 "Assistente de consulta". O Redis, sem catálogo, responde o dele pelo
adapter (`sample_key_patterns`).

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
  Promise **rejeita com `{ code, message }`**: o `Serialize` de `crate::Error`
  ([error.rs](src-tauri/src/error.rs)). `message` é o `Display` (legível, ex.:
  `"Connection error: ..."`); `code` é um de `STORAGE_ERROR`, `CONNECTION_ERROR`,
  `POOL_EXHAUSTED`, `CONNECTION_TIMEOUT`, `QUERY_ERROR`, `INVALID_QUERY`, `NOT_FOUND`,
  `ALREADY_EXISTS`, `INVALID_STATE`, `CANCELLED`, `UNSUPPORTED_DATABASE`,
  `UNSUPPORTED_TYPE`, `GUARD_REJECTED` (o guard do assistente recusou a consulta) ou
  `EXTERNAL_ERROR` (falha fora dos bancos — `pg_dump`, arquivo,
  updater —, com a mensagem sem prefixo). O assistente acrescenta
  `ASSISTANT_NOT_CONFIGURED` e os códigos da API dele como `ASSISTANT_*` (ver
  "Assistente de consulta" no §4). Rejeições do próprio Tauri (argumento que
  não desserializa, comando inexistente) e dos plugins continuam sendo string.
  Sempre use `try/catch`.

```ts
import { invoke } from '@tauri-apps/api/core';

try {
  const result = await invoke<QueryResult>('execute_query', {
    serverId: 1,
    database: 'postgres',
    query: 'SELECT * FROM users',
  });
} catch (err) {
  // err é { code: 'QUERY_ERROR', message: 'Query error: ...' }
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
  sslMode: SslMode;               // 'disable' | 'prefer' | 'require' | 'verify-ca' | 'verify-full' (ver "TLS das conexões", §1)
  sslRootCert: string | null;     // CA própria (PEM) dos verify-*; null = CAs do sistema
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
  sslMode?: SslMode | null;       // ajustado ao banco ao gravar (ver "TLS das conexões", §1)
  sslRootCert?: string | null;    // só guardada nos verify-*
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
Gerado em `src/api/types/generated/` (ts-rs) e reexportado por
`src/api/types/catalog.types.ts` (Rust: `models/catalog.rs` e `catalog/model.rs`).
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

// Dois schemas comparados (`catalog_compare_schemas`); `base` é a referência.
interface ColumnSpec { dataType: string; nullable: boolean; defaultValue: string | null } // sem o próprio schema
interface SchemaComparison {
  base: string; other: string;
  onlyInBase: string[]; onlyInOther: string[];   // tabelas (sem partições)
  common: number;                                // tabelas nos dois
  different: { table: string;                    // das comuns, as com colunas diferentes
               changes: { column: string; expected: ColumnSpec | null; actual: ColumnSpec | null }[] }[];
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
  hasCatalog: boolean;          // Postgres/Mongo/SQLite (catalog_*); Redis usa list_schemas_with_tables
  supportsStructureChanges: boolean; // Postgres e SQLite (criar/renomear/remover tabela e coluna)
  supportsDropCascade: boolean; // só Postgres
  supportsSchemaCompare: boolean; // só Postgres (catalog_compare_schemas)
  supportsBackup: boolean;      // só Postgres (pg_dump/pg_restore)
  supportsAtomicSave: boolean;  // Postgres/Mongo/SQLite (save_row_changes); Mongo só é atômico em replica set
  supportsAssistant: boolean;   // os quatro; o Redis pelo plano de passos, sem catálogo
  queryLanguage: 'sql' | 'mongo' | 'redis_plan'; // o que o assistente gera para o banco
}
```

As capabilities dependem **só do tipo do banco**: `get_db_capabilities` devolve a tabela
inteira (`{ dbType, capabilities }[]`), que o front busca uma vez (`useDbCapabilities`) e
consulta por servidor (`caps[server.dbType]`) — é por ela que a árvore, a palette e o
autocomplete decidem catálogo, edição de estrutura, backup e comparar schemas, em vez de
comparar `dbType`. Banco novo ou recurso novo = flag aqui, nos dois lados.

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
| `preview_structure_change` | `{ serverId, database, change: StructureChange }` | `string[]` — o SQL, sem aplicar *(Postgres e SQLite)* |
| `apply_structure_change` | `{ serverId, database, change: StructureChange }` | `StatementResult` — numa transação; recarrega o schema no catálogo aberto *(Postgres e SQLite)* |

> **Edição da estrutura (DDL) pela árvore:** `StructureChange` (`models/structure.rs`) é
> `createTable`, `renameTable`, `dropTable` (`kind` + `cascade`), `addColumn`,
> `alterColumn` (`original` → `updated`: só o que mudou vira statement) ou `dropColumn`.
> Os statements são montados no backend (`adapters/postgres/ddl.rs`,
> `adapters/sqlite/ddl.rs`, peças comuns em `adapters/ddl.rs`): identificadores citados,
> tipo validado (sem `;`, aspas ou comentário) e default parseado como **uma** expressão.
> No SQLite, mudar tipo/nulo/default de uma coluna **recria a tabela** (lang_altertable
> §7): a `CREATE TABLE` original é alterada só na coluna, os dados são copiados e índices e
> triggers voltam; foreign keys desligadas durante e conferidas antes do commit. A prévia
> roda numa transação desfeita (o rename que antecede a recriação precisa existir para ela).
>
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
| `catalog_compare_schemas` | `{ serverId, database, base, other }` | `SchemaComparison` | sim: só as colunas das tabelas comuns, pela faixa de prioridade (só Postgres) |
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
| `save_row_changes` | `{ serverId, database, editable, changes: RowChanges }` | `StatementResult` *(DELETE → INSERT → UPDATE numa transação)* |
| `cancel_query` | `{ serverId, database, queryId }` | `void` *(só Postgres; Mongo/Redis retornam "não suportado")* |

`StatementResult`: `{ affectedRows: number, executionTimeMs: number }`.

### Browse de tabela (paginado/ordenado/filtrado — sem digitar query)

| Comando | Args | Retorno |
|---|---|---|
| `fetch_table_data` | `{ serverId, database, request: TableDataRequest }` | `QueryResult` |
| `get_capabilities` | `{ serverId }` | `AdapterCapabilities` |
| `get_db_capabilities` | — | `{ dbType, capabilities }[]` — a tabela de todos os tipos |

### Exportação

| Comando | Args | Retorno |
|---|---|---|
| `write_export_file` | `{ path, contents }` | `void` |

O front serializa (CSV/JSON/SQL) e escolhe o caminho pelo `plugin-dialog`; o backend
só grava. JSON e SQL também podem ir para a área de transferência
(`plugin-clipboard-manager`) quando o resultado é pequeno. Para exportar além da
página carregada, reenvie `fetch_table_data` (ou `execute_query`) com
`unlimited: true`.

### Backup e restauração (Postgres — ferramentas nativas)

| Comando | Args | Retorno |
|---|---|---|
| `pg_tools_detect` | — | `PgToolsDetection` (instalação em uso + todas as achadas) |
| `pg_tools_status` | `{ serverId, database }` | `PgToolsStatus` (+ versão do servidor e `problem`) |
| `backup_preview` | `{ request: BackupRequest }` | `{ command, fileName }` |
| `backup_start` | `{ request, path }` | `jobId` |
| `restore_inspect` | `{ path }` | `RestoreInspection` (formato + resumo do `.dump`) |
| `restore_preview` | `{ request: RestoreRequest, path }` | linha de comando |
| `restore_start` | `{ request, path }` | `jobId` |
| `backup_cancel` | `{ jobId }` | `void` |

Tudo em `services/backup/` + `commands/backup.rs`. Não passa pela trait
`DatabaseAdapter`: o trabalho é de um processo externo (`pg_dump`, `pg_restore`, `psql`),
não do pool. Só Postgres.

- **Ferramentas** (`tools.rs`): a pasta das Preferências (`AppSettings.pgToolsDir`), se
  houver, é a única que vale — inválida vira `configuredError`, sem cair em outra. Sem
  ela: `PATH` e locais conhecidos (Homebrew, Postgres.app, EDB, MacPorts, PGDG, NixOS,
  `Program Files`) — um app aberto pelo Finder não tem o Homebrew no `PATH`. Escolhe a
  maior versão. O backup é bloqueado (`clientTooOld`) quando o `pg_dump` é de major
  anterior à do servidor; a restauração não.
- **Linha de comando** (`args.rs`, puro e testado): a **senha nunca vai no argv** (`ps`
  mostra) — sai por `PGPASSWORD`; com `connectionUri`, a URI vai sem a senha e com o
  database trocado. `PGSSLMODE` é o `sslMode` do servidor e, nos `verify-*`, `PGSSLROOTCERT`
  aponta a CA própria ou `system` (CAs do sistema — exige libpq 16+);
  `PG*` herdadas do app são limpas. Nomes vão entre aspas no `-n`/`-t` (`"Vendas"."pedidos"`).
  Combinações que o `pg_dump` recusaria ou ignoraria em silêncio viram erro.
- **Jobs** (`runner.rs`): um processo por job; stdout e stderr viram log em lotes (200 ms
  ou 200 linhas) no evento `backup-event` (`{ jobId, type: log | done | error | cancelled }`).
  O backup grava em `arquivo.partial` e renomeia só no sucesso (erro/cancelamento apagam).
  Sem "tudo ou nada", o `pg_restore` pula erros e sai com 1 — vira `done` com `warnings`.
  No fim de uma restauração o catálogo do database é revalidado (`ensure_fresh`).
- **Formato do arquivo** (`inspect.rs`): pelo conteúdo (`PGDMP` = custom), não pela
  extensão; `.gz`, tar e pasta recebem mensagem própria. O resumo vem do
  `pg_restore --list` (sem tocar em banco).
- **`SET transaction_timeout`**: o `pg_dump`/`pg_restore` 17+ escrevem essa linha sempre,
  e um servidor anterior ao 17 a recusa — no "tudo ou nada", a restauração inteira cai.
  Com ferramentas 17+ e servidor < 17 (`needs_timeout_filter`), a restauração passa pelo
  `psql --file=-` e o app filtra a linha: o `.sql` é lido pelo app; o `.dump` vem de
  `pg_restore --file=-` (só gera o SQL). Se a fonte falhar no meio, o `psql` é morto
  **antes** de ver o fim da entrada — um EOF faria ele confirmar o que chegou.
- **Limitações conhecidas**: `pg_dump -t` não leva o schema da tabela (nem tipos, funções
  e extensões) — o destino precisa tê-los; scripts do `pg_dump` 17.6+ trazem `\restrict`
  (comando do `psql`, outros clientes SQL não entendem). Formatos tar/diretório, jobs
  paralelos e dump global (roles) ficam de fora.

Testes: unidade sem nada (`cargo test --lib backup`, com `pg_dump` falso em shell para o
runner); ponta a ponta com o Postgres dos e2e e as ferramentas reais —
`cargo test --lib backup::e2e -- --ignored` (pasta por `OCTAPUS_PG_TOOLS_DIR` ou a
detecção normal; no macOS, `brew install libpq`).

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
`Default`, e `pnpm gen:types` para o tipo do front (gerado pelo ts-rs) — sem migração: o que
nunca foi salvo (ou ficou ilegível) volta com o padrão. O tema **não** mora aqui: fica
no `ui-store` (localStorage), lido antes da primeira pintura. `pgToolsDir` é a pasta das
ferramentas do Postgres (`null` = detectar sozinho — ver Backup e restauração).
`assistantAutoExecute` (desligado por padrão, RN-13): o assistente executa sozinho a
consulta de leitura que gerou; desligado, ela fica para revisão.

### Assistente de consulta (API do assistente)

| Comando | Args | Retorno |
|---|---|---|
| `assistant_status` | — | `AssistantStatus` `{ configured, online, loggedIn, minAppVersion, updateRequired }` — nunca rejeita por causa da API |
| `assistant_redeem` | `{ inviteCode, deviceLabel? }` | `AssistantParticipant` `{ id, label }` |
| `assistant_logout` | — | `void` (só apaga o token local; a API não tem logout) |
| `assistant_me` | — | `AssistantMe` (participante, `modelSelection`, versões do termo, cota) |
| `assistant_consent_current` | — | `AssistantConsentTerm` `{ version, body (Markdown), publishedAt }` |
| `assistant_consent_accept` | `{ version }` | `void`; versão que não é a vigente → `ASSISTANT_CONFLICT` |
| `assistant_models` | — | `AssistantModel[]` — só no modo `free` (senão `ASSISTANT_MODEL_SELECTION_FORBIDDEN`) |
| `assistant_conversations` | `{ cursor? }` | `AssistantConversationPage` (mais recentes primeiro) |
| `assistant_conversation` | `{ conversationId }` | `AssistantConversation` (turnos, tentativas, outcome, avaliação) |
| `assistant_conversation_create` | `{ serverId, modelId?, title? }` | `AssistantConversationSummary` — paradigma do banco do servidor; o modelo fica fixo |
| `assistant_turn_ask` | `{ serverId, database, conversationId, question, clientTurnId, schemas?, onEvent: Channel }` | `AssistantTurnFinish`; o andamento chega pelo canal (`AssistantTurnEvent`) |
| `assistant_turn_cancel` | `{ clientTurnId }` | `boolean` (`false` = já terminou) |
| `assistant_execute` | `{ serverId, database, query, page, attemptId?, userEdited }` | `QueryResult` — execução manual, sempre só-leitura |
| `assistant_feedback_set` | `{ turnId, feedback: AssistantFeedbackInput }` (`{ rating, reasons, comment, attemptId }`) | `void` — avalia o turno (👍/👎, B11); repetir sobrescreve |
| `assistant_feedback_clear` | `{ turnId }` | `void` — remove a avaliação (sem avaliação não é erro) |

O app fala com a API intermediária do TCC (octapus-assistant-api, contrato 1.4.0 em
`contract/` daquele repositório), que chama os provedores de modelo e registra o
experimento; o loop do assistente roda aqui. Módulo `assistant/`:

- **URL** (`config.rs`): `OCTAPUS_ASSISTANT_URL` no ambiente do app; sem ela, o valor do
  build (`option_env!`; o `build.rs` recompila quando a variável muda, e o `release.yml`
  passa `vars.OCTAPUS_ASSISTANT_URL`). `https`, ou `http` só em localhost. Sem URL o
  assistente fica desligado (`ASSISTANT_NOT_CONFIGURED`) e o resto do app não muda. O
  pacote Nix não recebe a variável, então sai sem assistente.
- **Cliente** (`client.rs`): o `reqwest` do projeto (rustls), Bearer com o token do
  participante, timeout de 20 s (5 s no health check). Erro da API (`{ error: { code,
  message } }`) vira `Error::Assistant` com o código `ASSISTANT_<CODE>` (`error.rs`);
  corpo fora do contrato (proxy) decide pelo status; rede/DNS/tempo esgotado e o
  `unavailable` da API viram `ASSISTANT_UNAVAILABLE` (C-19). O `Retry-After` de um 429
  fica no erro (`retry_after`), para o orquestrador esperar.
- **Token** (`storage/repositories/assistant.rs`): tabela `assistant_session` (linha
  única) com o token **cifrado pelo vault** e a URL da API que o emitiu — token de outra
  URL não é usado, para nunca ir a outro host. É a única coisa do assistente em disco
  (RN-01). Sem token, os comandos autenticados respondem `ASSISTANT_UNAUTHORIZED`; um
  401 da API (convite revogado) apaga o token.
- **Avaliação** (B11, A8 na API): `PUT`/`DELETE /v1/turns/{turnId}/feedback`, por turno.
  Antes de sair, `AssistantFeedbackInput::normalized` (`models/assistant.rs`) deixa como
  a API aceita: motivos só no 👎, sem repetir e sem `unknown`; comentário aparado (só
  espaços = `null`) e acima de 2000 caracteres (contados como a API, não em bytes) →
  `ASSISTANT_INVALID_REQUEST` sem ir à API. As recusas da API chegam com o código dela
  (`CONFLICT` = turno sem tentativa concluída, `CONSENT_REQUIRED`, `NOT_FOUND`…). A
  avaliação volta no `assistant_conversation` (`AssistantTurn.feedback`). `FeedbackReason`
  aceita valor novo da API como `unknown`, como os status.
- **Log:** só ids (participante, conversa, turno), códigos de erro e eventos (login, termo
  aceito, API fora, avaliação gravada/removida) — nunca pergunta, estrutura, consulta,
  comentário, mensagem da API ou token.

**Execução só-leitura** (RN-16). A consulta gerada passa por duas barreiras, e a do banco
segura sozinha (os testes rodam cada executor com o guard desligado):

- **Guard** (`assistant/guard/sql.rs`, Rust puro sobre o `sqlparser`, falha fechado): a
  consulta precisa parsear e ser exatamente um statement `Query`; uma varredura dos tokens
  recusa, em qualquer profundidade, `INSERT`/`UPDATE`/`DELETE`/`MERGE` (CTE que modifica
  dados), `INTO`, `FOR UPDATE/SHARE`, identificador `U&"…"` e funções da denylist — o que a
  transação `READ ONLY` não impede: sessões (`pg_terminate_backend`, `set_config`), lock
  consultivo, arquivos do servidor, `dblink`, SQL dinâmico (`query_to_xml`), sequências,
  `lo_*`; no SQLite, `load_extension`/`readfile`/`writefile`. O que o parser não entende
  (ex.: `TABLE t`) é recusado. Recusa → `GUARD_REJECTED`; linguagem que não é a do banco
  (ou sem guard) → `UNSUPPORTED_TYPE`. Os guards do Mongo e do Redis estão em "MongoDB" e
  "Redis" logo abaixo.
- **Executor** (`DatabaseAdapter::execute_read_only(op, page)`, `ReadOnlyOp` em
  `adapters/read_only.rs`): a consulta vai dentro de `SELECT … FROM (\n…\n)` com
  `LIMIT`/`OFFSET` da página (teto de 1.000 linhas, `ReadOnlyLimits`) e o resultado nunca é
  editável. **Postgres:** conexão do pool presa à operação, `BEGIN READ ONLY`, `SET LOCAL`
  de `statement_timeout` (30 s), `lock_timeout` (2 s) e `search_path`, protocolo estendido
  (um statement só: `COMMIT; DROP …` não passa), cada coluna em `::text` e `ROLLBACK`
  sempre; `queryId` liga o `cancel_query`. **SQLite:** `PRAGMA query_only` durante a
  execução (restaurado no fim), um statement só (o `prepare` do rusqlite ignoraria o resto
  em silêncio, então o texto passa pelo `Batch`) e um vigia que interrompe a consulta no
  tempo limite.
- **Preflight** (`DatabaseAdapter::explain_read_only`, `assistant::read_only::preflight`):
  as mesmas barreiras, com `EXPLAIN (FORMAT JSON)` sem `ANALYZE` no Postgres (devolve
  `ReadOnlyPlan` com linhas e custo estimados) e `EXPLAIN QUERY PLAN` no SQLite (só os
  passos). Acha tabela, coluna ou tipo errado sem executar nada.
- **Falha da consulta** (`assistant/failure.rs`), o que vai para a API no outcome e na
  correção: `classify` dá a `FailureClass` do contrato pelo SQLSTATE do Postgres (o
  caminho só-leitura devolve `Error::DbQuery { sqlState, message }`, que para o front é
  igual ao `QUERY_ERROR`) ou pela mensagem do SQLite; `None` = o erro não é da consulta
  (conexão, cancelamento) e o turno para. `sanitize` (RN-10) manda só o SQLSTATE e a
  mensagem principal — nunca `DETAIL`, `HINT` ou `WHERE` — com cada trecho entre aspas e
  cada número trocados por `?`, exceto o que aparece como token na própria consulta
  gerada (nome de coluna, literal da pergunta); no SQLite, o que vem depois do `: `
  também. No máximo 2.000 caracteres.
- `assistant::read_only::execute` junta guard e executor (e o `preflight`, guard e plano)
  — é o que o orquestrador do turno chama. A operação vem como `ReadOnlyOp::new(language,
  texto)`, com a `queryLanguage` das capabilities do banco.

**MongoDB** (B8). A consulta é a operação do contrato (`mongo-operation.schema.json`): um
JSON com `collection`, `operation` (`find`, `aggregate`, `countDocuments`, `distinct`) e
os campos de cada uma, valores em Extended JSON relaxado. O parse é estrito
(`adapters/mongo/operation.rs`: campo desconhecido, tipo errado ou campo que não serve
para a operação é recusado) e é o mesmo para o guard e para o executor. O `kind: code` do
Mongo vem como comando do shell (`db.x.updateMany(…)`) e nunca roda.

- **Guard** (`assistant/guard/mongo.rs`, falha fechado): operação de leitura pelo mesmo
  `MongoCommand::is_write` do editor; estágios do pipeline por **allowlist**, também nos
  sub-pipelines de `$lookup`, `$unionWith` e `$facet` (`$out`/`$merge` escrevem; os de
  administração, como `$currentOp`, `$collStats`, `$changeStream`, e estágio
  desconhecido ficam de fora); `$where`, `$function`, `$accumulator` e valor BSON de
  código em qualquer profundidade; coleções `system.*`, com `$` ou de outro database
  (`from: {db, coll}`). As mensagens nunca repetem valor do filtro.
- **Executor** (`adapters/mongo/read_only.rs`): o Mongo não tem transação de leitura,
  então o comando sai do tipo da operação (nada de texto do shell) e um pipeline com
  `$out`/`$merge` é recusado ali mesmo, sem o guard. `maxTimeMS` de 30 s em todo comando
  (mais 5 s de teto no app), página por `skip`/`limit` dentro do `skip`/`limit` que o
  modelo pediu (`find`) ou por `$skip`/`$limit` no fim do pipeline, teto de 1.000
  documentos, resultado sem `editableInfo`. O cancelamento larga a operação (o driver não
  tem cancel request) e o servidor para no `maxTimeMS`.
- **Preflight:** confere que as coleções lidas existem (no Mongo, ler coleção que não
  existe devolve vazio, sem erro: o preflight responde `NamespaceNotFound`) e roda
  `explain` com `verbosity: "queryPlanner"`, que planeja sem executar. O `ReadOnlyPlan`
  traz só o caminho do plano vencedor (`IXSCAN status_1 → FETCH`), sem estimativa.
- **Falha:** os erros do servidor saem como `Error::MongoQuery { code, codeName, message }`
  (`QUERY_ERROR` para o front). `classify` decide pelo código (50/262 tempo, 26 objeto,
  14/241 tipo, `BadValue`/`FailedToParse`/estágio desconhecido sintaxe) e, nos códigos
  genéricos de cada operador, pela mensagem. `sanitize` manda `CodeName (n): mensagem`
  com a mesma troca por `?`, e os tokens conhecidos vêm do JSON da operação (chaves,
  textos sem o `$`, partes de caminho, números). Aspa colada numa palavra (apóstrofo,
  aspa dentro do valor) não abre nem fecha o trecho — o que vale também para o SQL.
- **Estrutura** (`adapters/mongo/shape.rs` + `assistant/shape_context.rs`): ver o
  contexto da estrutura abaixo.

**Redis** (B9). O modelo não vê chaves nem valores (RN-09), então a consulta é um **plano
de passos** (`redis-plan.schema.json`): até 8 passos `{ id, command, args, foreach }` e o
`output`. Um passo com `foreach` roda uma vez por item do resultado de um passo anterior,
com `$item` trocado pelo item nos args (também dentro de um texto: `$item:cart`). O parse
é estrito e o mesmo para guard e executor (`adapters/redisdb/plan.rs`: campo a mais, id
inválido ou repetido, `foreach` para passo que não veio antes, `$item` sem `foreach` e
`output` que não existe são recusados). O `kind: code` do Redis vem como comandos do
`redis-cli`, um por linha, e nunca roda.

- **Guard** (`assistant/guard/redis.rs`, falha fechado): todo passo usa um comando da
  **allowlist de leitura** (`plan::READ_COMMANDS`: keyspace, strings, hashes, listas, sets,
  sorted sets, `XRANGE`/`XLEN`, geo); `SORT` (tem `STORE`), `PFCOUNT` (grava o cache do
  HLL), módulos e o que não está na lista ficam de fora. Motivos próprios: `keys` (`KEYS`),
  `script` (`EVAL`, `EVALSHA`, `FCALL`, `FUNCTION`, `SCRIPT`), `write` (escritas
  conhecidas, só para o motivo), `not_allowed` (administração, bloqueantes, `SELECT`,
  `MULTI`…) e `scan_in_foreach` (uma varredura do keyspace por item). As mensagens nunca
  repetem argumento.
- **Executor** (`adapters/redisdb/read_only.rs`): o Redis não tem modo só-leitura por
  conexão, então cada passo passa de novo pela allowlist antes de ir ao servidor. O app
  percorre o cursor do `SCAN` (o do modelo, se vier, tem que ser `0`; aceita `MATCH`,
  `COUNT` e `TYPE`) e tira as chaves repetidas. Só roda o que leva ao `output`; o
  `foreach` que é o `output` roda só para os itens da página (pipelines de 100). Tetos:
  10.000 chaves por `SCAN`, 1.000 itens num `foreach` que não é o `output`, 5.000
  comandos por execução e 30 s no plano inteiro. O `SCAN` que vai para a tela (ou
  alimenta o `output`) só corta no teto — a página sai do que foi lido e, sem total,
  `hasMore` fica ligado; pedir a página depois do que foi lido é falha. O que alimenta
  outro `foreach` precisa do keyspace inteiro e falha no teto. A tabela: `SCAN` → uma
  chave por linha; resposta em pares → `member`/`score` ou `field`/`value`; `foreach` →
  o item (`key` quando a fonte é `SCAN`) e a resposta, com `HGET`/`HMGET` nomeando as
  colunas pelos campos e `HGETALL` abrindo uma coluna por campo (até 50). `totalCount`
  vai quando se sabe (resposta inteira, `SCAN` até o fim): é a resposta das perguntas de
  "quantos". O `cancel_query` não interrompe o Redis: o turno larga a execução.
- **Preflight:** parse, allowlist, opções do `SCAN` e, pelo `COMMAND INFO` dos comandos do
  plano, que cada um existe nesta versão do servidor, que o número de argumentos bate com
  a aridade (`ERR wrong number of arguments for 'hget' command`, como o Redis) e que o
  servidor não o marca como `write`/`admin`/`blocking`. Nada do plano roda. Servidor que
  esconde o `COMMAND` (alguns gerenciados) pula essa conferência. O `ReadOnlyPlan` traz só
  os comandos (`SCAN → foreach HGET`), sem estimativa.
- **Falha:** os erros do servidor saem como `Error::RedisQuery { code, message }` (o código
  da resposta: `WRONGTYPE`, `ERR`, `NOPERM`…) e os tetos do executor como
  `Error::PlanLimit` — os dois `QUERY_ERROR` para o front. `classify`: `WRONGTYPE` e
  valor que não é número → `type_mismatch`; comando desconhecido, aridade e sintaxe →
  `syntax`; `READONLY` (réplica recusou escrita) → `guard_rejected`; `PlanLimit` →
  `paradigm_limitation` (o paradigma não expressa o pedido sem ler demais; a separação
  fina de RN-30 fica na planilha); o resto, `runtime_error`. `sanitize` manda
  `CODE: mensagem` com a mesma troca por `?` (a chave que entrou no `$item` é dado do
  banco e não é token do plano, então vira `?`); o `PlanLimit` é texto nosso, só com id de
  passo, comando e os tetos, e vai inteiro.
- **Estrutura** (`adapters/redisdb/patterns.rs` + `assistant/key_context.rs`): ver o
  contexto da estrutura abaixo.

**Contexto da estrutura** (`assistant/schema_context.rs`, RN-26): o DDL que vai ao modelo
com a pergunta, sem nunca carregar a estrutura inteira.

- **Quais relações:** `Catalog::context_candidates` (Rust puro, `catalog/query.rs` e
  `catalog/context.rs`) ranqueia as relações dos schemas da conversa pelo casamento léxico
  dos nomes com a pergunta e o histórico (que pesa metade): minúsculas sem acento,
  singular aproximado, partes de `snake_case`/`camelCase`, nome inteiro > parte > prefixo
  comum de 5 letras (`eventos` acha `event_log`). Até 30 candidatas vão todas; acima
  disso, as 10 melhores mais as vizinhas pelas chaves estrangeiras (`with_neighbors`), até 30.
- **Da fonte, só das escolhidas** (`CatalogSource::foreign_keys` e `relation_details`,
  pela faixa de prioridade do `CatalogService`): chaves estrangeiras entre relações do
  escopo (no Postgres pelo índice de `conrelid`, com teto de 1.000) e colunas com tipo,
  nulo, chave primária e comentários. Mongo e Redis: `UNSUPPORTED_TYPE`.
- **Mongo** (`assistant/shape_context.rs`): sem colunas nem chaves estrangeiras. As
  coleções vêm do mesmo ranking (`context_candidates` no `FLAT_SCHEMA`) e só as 12
  primeiras são amostradas por `CatalogSource::infer_shape`: um `$sample` de 100
  documentos por coleção, na conexão dedicada do catálogo (duas por vez, `maxTimeMS` de
  3 s; a que falha fica só pelo nome). `shape::infer` tira caminhos aninhados
  (`customer.name`, `items[].sku`), os tipos BSON vistos e a % de presença, e **descarta
  os documentos ali mesmo** (RN-08). Chave que parece dado vira `{key}`: pelo formato
  (número, UUID, ObjectId/hex, e-mail, data, mais de 64 caracteres) ou pelo uso (objeto
  com mais de 20 chaves, cada uma em menos da metade dos documentos, é mapa). Até 120
  campos por coleção (ficam os mais presentes), 6 níveis. Formato:
  `collection "orders" (100 sampled documents):` e uma linha por campo
  (`"items[].qty" int|double 100%`), no mesmo orçamento e com a mesma linha final
  (`-- Other collections (fields not shown): …`). O `list_columns` da árvore não muda.
- **Redis** (`assistant/key_context.rs`): não tem catálogo, então o `assistant_turn_ask`
  desvia pelas capabilities (`hasCatalog`) e o contexto sai do próprio adapter
  (`DatabaseAdapter::sample_key_patterns`, a cada pergunta). `patterns::sample` faz um
  `SCAN` amostrado (o `scan_keys` da árvore, até 10.000 chaves, com teto de 5 s), o
  `DBSIZE` e o `TYPE` de até 3 chaves por padrão (num pipeline, até 200 padrões, os de
  mais chaves) — **as chaves lidas morrem ali** (RN-09). `generalize` divide nos `:` e
  só mantém o segmento com cara de nome (começa com letra; letras, até dois dígitos, `_`
  e `-`; até 40 caracteres; não parece hex nem token): número, UUID, hex, e-mail, data,
  token, espaço ou caractere de glob viram `{id}`, inteiros ou na parte que varia
  (`user12345` → `user{id}`, `order_2024_001` → `order_{id}`); uma posição com mais de 8
  nomes diferentes sob o mesmo prefixo também vira `{id}`. Formato: o cabeçalho diz se a
  amostra cobriu o banco (`-- Redis. Key patterns in a sample of 10000 of 152340 keys`),
  uma linha por padrão (`"user:{id}" hash, 1203 keys`), ranqueados pelas palavras da
  pergunta e pelo número de chaves, no mesmo orçamento e com a linha
  `-- Other key patterns (not shown): …`. Amostra que não cobriu o banco liga `truncated`.
- **Formato:** `CREATE TABLE "schema"."tabela" (…)` com `NOT NULL`, `PRIMARY KEY`,
  `REFERENCES` (compostas numa linha própria), views como `CREATE VIEW`, em ordem de
  schema e nome. Nomes sempre entre aspas (caractere de controle vira `\u{…}`) e
  comentários entre `«»` numa linha, cortados em 200 caracteres — dado, nunca instrução
  (RN-22).
- **Orçamento:** um oitavo do `contextWindow` do modelo, entre 1.000 e 16.000 tokens
  (estimativa: 4 caracteres por token). As escolhidas entram por prioridade; a primeira
  entra mesmo grande, com colunas cortadas; as que sobram vão só pelo nome numa última
  linha, com espaço reservado. Qualquer corte liga `truncated` (`schemaTruncated`).
- Desempenho: `context_candidates` está no `perf::catalog_core` (p95 ≈ 0,1 ms num tenant
  do catálogo de 5.000 × 150).

**Prompts, mensagens e resposta** (B5):

- **Prompts** em `src-tauri/prompts/<paradigma>-v<n>.txt` (`postgres-v1`, `sqlite-v1`,
  `mongo-v1`, `redis-v1`),
  embutidos com `include_str!` (`assistant/prompts.rs`). A API confere o SHA-256 dos bytes
  exatos da mensagem `system` contra o prompt registrado com o mesmo id: registre **o
  próprio arquivo** no dashboard (Prompts → enviar arquivo, com o paradigma). Texto
  mudou = arquivo novo (`-v2`), registrado antes do release; o teste
  `prints_and_pins_each_prompt_hash` imprime os hashes (`cargo test --lib
  assistant::prompts -- --nocapture`) e reprova se um arquivo for editado. O
  `.gitattributes` fixa LF nos prompts e no contrato (CRLF no Windows mudaria o hash).
- **Mensagens** (`assistant/messages.rs`, `attempt_request`): a `system` (nada variável vai
  nela); os turnos anteriores (`<question>` + o JSON da resposta), os mais recentes que
  cabem em metade do orçamento da estrutura (C-12); o turno atual com `<schema>` (ou
  `<schema truncated="true">`) e `<question>`; em correção, cada tentativa que falhou e
  um `<previous_attempt_failed stage=… class=…>` com o erro sanitizado (C-11). A última é
  sempre do usuário. Mudar esse formato também é versão nova do prompt.
- **Resposta** (`assistant/output.rs`): `ModelOutput` do contrato, conferido de novo no
  app (os seis campos, as regras de cada `kind`, e a `language` tem que ser a do banco);
  fora disso é `invalid_output`. Cópia do contrato em `src-tauri/contract/` (os três
  schemas JSON), com o SHA-256 de cada um fixado em teste para a versão 1.4.0
  (`CONTRACT_VERSION`); com `OCTAPUS_CONTRACT_DIR` apontando para o `contract/` da API, o
  teste compara byte a byte.
- **Idioma da pergunta** (`assistant/language.rs`, crate `whatlang`): `pt`, `en` ou
  `other`, gravado no turno (C-02).

**O turno** (B6, `assistant/turn.rs`, `run_turn`): uma máquina de estados que depende de
duas portas — `TurnApi` (a API) e `TurnDatabase` (o banco, só leitura) —, o que deixa os
testes rodarem com as duas simuladas. O comando `assistant_turn_ask` monta o `TurnSetup`
(conversa e histórico da API, contexto da estrutura, prompt, preferência de execução
automática) e liga as portas reais.

1. Abre o turno (`clientTurnId`: repetir é seguro) e avisa a tela.
2. Pede a tentativa: `POST /attempts`, lendo o SSE (`assistant/sse.rs`) com timeout de
   inatividade de 45 s; 429 espera o `Retry-After` (até 3 vezes) e avisa (`waiting`).
3. Confere a resposta (`output::parse`). `clarification` encerra; `code` é entregue e
   **nunca** executado (RN-14); saída inválida (da API ou do app) tenta de novo com
   `previousFailure: invalid_output`.
4. `query`: guard → preflight → execução (só com `assistantAutoExecute`; desligada, a
   consulta e o plano ficam para a revisão). Falha da consulta → outcome com a classe e o
   erro sanitizado → nova tentativa com a resposta que falhou e o erro (C-11). Falha que
   não é da consulta (conexão, banco fora) encerra o turno como `failed`.
5. A API corta em 4 tentativas (`turn_limit_exceeded`; o app tem teto próprio de 6):
   `exhausted`, entregando a última consulta gerada (RN-24).
6. Cada tentativa com resposta ganha um outcome; o turno termina com `finish`. Para `code`
   e `clarification` o guard não roda: vai `allowed: false` com o motivo `kind_code` ou
   `kind_clarification`, separado da recusa de verdade na análise.

Cancelar (`assistant_turn_cancel`, pelo `clientTurnId`) acorda quem está esperando: fecha
o stream (a API interrompe o provedor e devolve a cota) ou chama o `cancel_query` da
execução marcada com o `clientTurnId`, e o turno termina como `cancelled`. A execução
manual (`assistant_execute`, revisão e páginas seguintes) passa pelo mesmo caminho
só-leitura; com `attemptId`, a primeira página completa o outcome guardado em memória
(nunca em disco) e manda de novo. E2E com a API de verdade (gasta tokens):
`OCTAPUS_E2E_ASSISTANT_URL=… OCTAPUS_E2E_INVITE=… cargo test --lib
commands::assistant::e2e -- --ignored --nocapture`. E2E: `cargo test --lib read_only -- --include-ignored`
  (Postgres e Mongo dos e2e; o SQLite usa arquivo temporário) e `cargo test --lib
  shape_context -- --ignored` (contexto do Mongo com canários).

**Prova de privacidade** (B10, critérios 7 e 8 do épico,
`commands/assistant_privacy_e2e.rs`): um e2e por banco (Postgres, SQLite, Mongo e Redis)
semeia **canários** — strings únicas da execução nos valores, nos campos de texto livre,
nas chaves do Redis com cara de registro (UUID, número, hex, e-mail, nomes demais numa
posição) e nas chaves de mapa do Mongo — e roda os fluxos contra a API local: contexto da
estrutura, erro do banco que cita o valor (`amount::numeric`, `$toDouble`) e a correção,
pergunta de seguimento, execução manual com paginação e com erro, e avaliação com
comentário. Depois procura cada canário (1) no que o app mandou à API (a `Recording` do
módulo `e2e` grava cada chamada da `TurnApi`), (2) no log do app, capturado com os níveis
do arquivo de log (`logging::app_level`/`DEPENDENCIES_LEVEL`), (3) em todas as tabelas do
banco da API, numa transação só de leitura, e (4) no `docker logs` da API. Nenhum pode
aparecer; e cada teste confere que o canário chegou à tela (o erro como o banco deu, as
linhas), para a prova não ser vazia. O de Postgres também resgata o convite num cliente
novo e lê a conversa e a avaliação: o histórico volta noutra máquina só com o convite.
Além das variáveis do e2e, pede `OCTAPUS_E2E_API_DATABASE_URL` e
`OCTAPUS_E2E_API_CONTAINER`: `cargo test --lib commands::assistant::privacy_e2e --
--ignored --nocapture` (Mongo em 27017, Redis em 6380 db 3, Postgres em 5432 com um banco
próprio que o teste cria e apaga). No disco, `storage::repositories::assistant` trava o
esquema: a única tabela do assistente no `app.db` é a do token. Limites declarados (não
são falha): uma chave de Mongo ou Redis que é dado mas tem cara de nome em poucas chaves
(`user:ana.silva`) sai como literal, e uma chave do Redis sem `{id}` (`leaderboard`) sai
inteira.

### Diagnóstico

| Comando | Args | Retorno |
|---|---|---|
| `open_log_dir` | — | `void` — abre a pasta de logs com o `octapus.log` selecionado (Preferências › Diagnóstico) |

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
Para salvar tudo junto, use `save_row_changes` (quando `supportsAtomicSave`): ele aplica
`deletes` → `inserts` → `edits` numa transação só e, se uma etapa falha, desfaz as outras
— a grade pode reenviar as mesmas pendências sem que uma parte já tenha valido:
```ts
await invoke<StatementResult>('save_row_changes', {
  serverId, database, editable: data.editableInfo,
  changes: { deletes: [['42']], inserts: [{ values: [['name', 'Ana']] }], edits },
});
```
No Mongo a transação exige replica set ou mongos (o backend pergunta ao `hello`); num
standalone as etapas valem uma a uma, como nas chamadas separadas, com aviso no log. Sem
`supportsAtomicSave`, a ordem ao salvar é `delete_rows` → `insert_rows` →
`apply_row_edits`, cada uma na sua transação. Depois, refetch.

---

## 7. Camada de acesso sugerida no front

Centralize os `invoke` num módulo tipado — evita repetir nomes e facilita tratar
o erro em um lugar só (o app faz isso em [client.ts](src/api/client.ts), com `ApiError`):

```ts
// src/api/db.ts
import { invoke } from '@tauri-apps/api/core';

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    // { code, message } dos comandos do app; string nas rejeições do Tauri
    const message = typeof e === 'string' ? e : (e as { message?: string }).message;
    throw new Error(message ?? 'Erro desconhecido no backend');
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
