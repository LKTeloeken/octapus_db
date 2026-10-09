# Arquitetura — Visão Geral

Visão de alto nível do **octapus_db** ligando frontend e backend. Para detalhes,
veja [BACKEND.md](BACKEND.md) e [FRONTEND.md](FRONTEND.md).

## Mapa do sistema

```
┌──────────────────────────── Frontend (React 19) ────────────────────────────┐
│  features/ (telas)  →  queries/ (React Query)  →  api/ (invoke tipado)        │
│        ↑ stores/ (Zustand: abas, árvore, tema)                                │
└───────────────────────────────────┬──────────────────────────────────────────┘
                                     │  invoke('comando', { ...camelCase })
                                     │  ▲ evento 'catalog-event' (progresso do catálogo)
                                     ▼
┌──────────────────────────── Backend (Rust / Tauri 2) ───────────────────────┐
│  commands/   handlers #[tauri::command] — validam e delegam                   │
│  services/   orquestração (ConnectionService = cache de pools, QueryService,  │
│              CatalogService = catálogo de metadados por database)             │
│  catalog/    núcleo do catálogo: modelo, formatos, busca, drift, disco        │
│  adapters/   trait DatabaseAdapter → postgres | mongo | redisdb | sqlite      │
│              + fontes do catálogo (introspect.rs de postgres, mongo, sqlite)  │
│  storage/    SQLite local (servidores, sessão) + vault (senhas cifradas)      │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Fronteira front ↔ back

- **Transporte:** Tauri `invoke`. Comando em `snake_case`; argumentos e retornos em
  **`camelCase`** (conversão automática). Erro = a Promise **rejeita com uma string**.
- **Eventos:** no sentido contrário, o backend emite `catalog-event` com o progresso dos
  catálogos (sincronizando, schemas, relações, pronto, erro); o front escuta em
  [api/catalog.ts](src/api/catalog.ts) e invalida só as fatias afetadas.
- **Contrato:** todo valor de célula trafega como `string | null`; formatação é do front.
- **Camada única no front:** `src/api/` centraliza os `invoke` ([client.ts](src/api/client.ts)
  normaliza o erro-string em `ApiError`); `src/queries/` envolve cada comando num hook
  React Query. Telas nunca chamam `invoke` direto.

## Decisões de design

### Adapters intercambiáveis
Cada banco implementa a trait `DatabaseAdapter` ([adapters/traits.rs](src-tauri/src/adapters/traits.rs)).
Métodos não suportados têm default que retorna `UnsupportedType`, então cada adapter só
implementa o que oferece. O front descobre o que renderizar via `get_capabilities` —
por isso a UI é uniforme para os quatro bancos.

### Conexão lazy + cache de pools
Não existe "abrir conexão" explícito. O `ConnectionService`
([services/connection.rs](src-tauri/src/services/connection.rs)) mantém um cache de
adapters por `(serverId, database)`; o primeiro comando cria o pool e os seguintes o
reutilizam. O helper `connect_adapter` ([commands/mod.rs](src-tauri/src/commands/mod.rs))
só busca o servidor (e a senha) **no cache-miss**.

### Catálogo de metadados no backend
A **estrutura** dos bancos (schemas, tabelas, coleções) não viaja inteira para o front.
O backend é o dono dela: um `CatalogService`
([services/catalog.rs](src-tauri/src/services/catalog.rs)) mantém um catálogo por
`(serverId, database)` e o front pede **fatias** — os filhos de um nó em janelas
paginadas e filtradas, a busca da palette, a resolução de um nome sem schema. Vale para
Postgres, Mongo e SQLite; o Redis segue com a estrutura inteira (chave não tem estrutura
fixa).

Por quê: num banco multi-tenant (5.000 schemas × 150 tabelas) a listagem inteira, com o
tamanho de cada tabela, levava o backend ao OOM e travava a árvore para sempre; no front,
a estrutura de 48 MB travava a UI por mais de um segundo a cada sugestão do autocomplete.
Os números estão em [perf/catalog/BASELINE.md](perf/catalog/BASELINE.md).

- **Introspecção em camadas** ([adapters/postgres/introspect.rs](src-tauri/src/adapters/postgres/introspect.rs)):
  primeiro os nomes dos schemas (a árvore já aparece), depois as relações. Schemas com o
  mesmo conjunto de tabelas (o mesmo **formato**) são lidos uma vez só — 5.000 tenants
  viram poucos formatos. Transação somente leitura com `statement_timeout`/`lock_timeout`:
  uma migração com lock nunca trava a árvore. Mongo lista coleções com `nameOnly`; SQLite
  lê o `sqlite_schema`.
- **Conexão própria e duas faixas**: uma para sincronizar em segundo plano, outra para o
  que o usuário espera agora (abrir um schema ainda não carregado, o tamanho da tabela
  aberta). Pedidos urgentes simultâneos viram um lote.
- **Revalidação por evento** (abrir o database, refresh), nunca por polling; um
  *fingerprint* por schema diz o que mudou.
- **Disco**: o catálogo é salvo cifrado com a chave do vault e volta na próxima abertura
  (a árvore aparece antes de ir ao banco).
- **Eventos**: o progresso chega ao front pelo evento Tauri `catalog-event`, que invalida
  só as fatias daquele database.
- **Escala na tela**: árvore em janelas de 500 com filtro, agrupamento por formato com
  aviso de *drift* (tenant com migração pendente), escopo salvo por conexão (padrões de
  databases/schemas visíveis) e "Copiar diagnóstico".

O roteiro e as medições de cada fase estão em [perf/catalog/](perf/catalog/README.md).

### Segredos / senhas
As senhas são criptografadas por um **cofre próprio do app**
([storage/vault.rs](src-tauri/src/storage/vault.rs)) — AES-256-GCM com uma chave
**presa ao dispositivo**, gerada na 1ª execução e guardada em
`<app_data_dir>/vault.key` (permissão `0600`). O ciphertext fica na própria coluna
`servers.password` do SQLite; nada vai para o cofre do SO.

- **Zero prompts e multiplataforma:** não usa Keychain/Secret Service em uso normal, então
  o comportamento é idêntico em macOS/Windows/Linux e não há diálogos de autorização.
- **Trade-off:** é criptografia *em repouso*. Protege contra o `app.db` ser
  copiado/sincronizado **sem** o `vault.key`, mas não contra um atacante local com acesso
  aos dois arquivos. (Proteção forte exigiria uma master password — não implementada.)
- **Migração do Keychain antigo:** instalações anteriores guardavam a senha no cofre do SO
  via crate `keyring` ([storage/secrets.rs](src-tauri/src/storage/secrets.rs), agora
  só-migração). Na 1ª conexão de cada servidor, a senha legada é lida do Keychain (um
  último prompt do SO), re-criptografada no vault e removida do Keychain.
- **Leitura sob demanda:** a senha só é decifrada ao **criar** um pool (cache-miss);
  comandos sobre uma conexão já aberta não tocam no cofre.

### Estado: servidor vs. UI
- **React Query** (`src/queries/`) é a fonte da verdade dos dados do backend (servidores,
  fatias do catálogo, colunas, dados de tabela). Alguns domínios de metadados (servidores,
  capacidades, databases, colunas, índices e a estrutura do Redis) são **persistidos em IndexedDB**
  ([providers/query-provider.tsx](src/providers/query-provider.tsx)); o catálogo não — o
  backend já o guarda em disco. Dados de tabela são sempre ao vivo.
- **Zustand** (`src/stores/`) guarda só estado de UI: abas abertas, nós expandidos da
  árvore, tabelas recentes, tema. Mutações invalidam as query keys relevantes
  ([queries/keys.ts](src/queries/keys.ts)) para disparar refetch.

## Fluxo típico (abrir uma tabela)

1. Usuário expande um servidor → `list_databases` → `connect_adapter` cria o pool
   (lê a senha do cofre **uma vez**). Só os nomes; milhares de databases ganham filtro e
   janela na árvore.
2. Expande um database → `catalog_open` abre o catálogo (do disco, se houver, e revalida
   em segundo plano) e `catalog_children` traz a primeira janela de schemas. Os eventos
   `catalog-event` atualizam a árvore conforme as camadas chegam.
3. Expande um schema → `catalog_children` da memória do backend; se a sincronização ainda
   não chegou nele, a faixa de prioridade carrega só esse schema.
4. Clica numa tabela → abre aba de browse → `fetch_table_data` monta o SELECT/find/scan
   no backend e pagina, devolvendo um `QueryResult`. Postgres aceita `whereExpr`.
5. Ordenar/filtrar/paginar → reenvia `fetch_table_data` com novo `TableDataRequest`.
6. Editar/inserir/remover linhas → alterações ficam pendentes no front e são aplicadas
   em lote no salvar (`apply_row_edits` / `insert_rows` / `delete_rows`), seguido de
   refetch via invalidação.
