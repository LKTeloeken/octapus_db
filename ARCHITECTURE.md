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
                                     ▼
┌──────────────────────────── Backend (Rust / Tauri 2) ───────────────────────┐
│  commands/   handlers #[tauri::command] — validam e delegam                   │
│  services/   orquestração (ConnectionService = cache de pools, QueryService)  │
│  adapters/   trait DatabaseAdapter → postgres | mongo | redisdb               │
│  storage/    SQLite local (servidores) + secrets (keychain do SO)             │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Fronteira front ↔ back

- **Transporte:** Tauri `invoke`. Comando em `snake_case`; argumentos e retornos em
  **`camelCase`** (conversão automática). Erro = a Promise **rejeita com uma string**.
- **Contrato:** todo valor de célula trafega como `string | null`; formatação é do front.
- **Camada única no front:** `src/api/` centraliza os `invoke` ([client.ts](src/api/client.ts)
  normaliza o erro-string em `ApiError`); `src/queries/` envolve cada comando num hook
  React Query. Telas nunca chamam `invoke` direto.

## Decisões de design

### Adapters intercambiáveis
Cada banco implementa a trait `DatabaseAdapter` ([adapters/traits.rs](src-tauri/src/adapters/traits.rs)).
Métodos não suportados têm default que retorna `UnsupportedType`, então cada adapter só
implementa o que oferece. O front descobre o que renderizar via `get_capabilities` —
por isso a UI é uniforme para os três bancos.

### Conexão lazy + cache de pools
Não existe "abrir conexão" explícito. O `ConnectionService`
([services/connection.rs](src-tauri/src/services/connection.rs)) mantém um cache de
adapters por `(serverId, database)`; o primeiro comando cria o pool e os seguintes o
reutilizam. O helper `connect_adapter` ([commands/mod.rs](src-tauri/src/commands/mod.rs))
só busca o servidor (e a senha) **no cache-miss**.

### Segredos / senhas
As senhas são criptografadas por um **cofre próprio do app**
([storage/vault.rs](src-tauri/src/storage/vault.rs)) — AES-256-GCM com uma chave
**presa ao dispositivo**, gerada na 1ª execução e guardada em
`<app_data_dir>/vault.key` (permissão `0600`). O ciphertext fica nas próprias colunas
`servers.password` e `servers.connection_uri` do SQLite; nada vai para o cofre do SO.

- **A URI de conexão também é segredo:** `mongodb+srv://user:senha@…` carrega credencial,
  então passa pelo mesmo cofre. Diferente da senha (que nunca é serializada), a URI **volta
  para o front redigida** — `mongodb+srv://user:••••@cluster.net`. Se o campo voltar
  intocado num `update_server`, o backend reconhece a máscara e preserva o valor guardado;
  qualquer texto diferente é tratado como URI nova. `get_by_id` é o único caminho que
  devolve a URI inteira, e é o que alimenta os adapters.

- **Zero prompts e multiplataforma:** não usa Keychain/Secret Service em uso normal, então
  o comportamento é idêntico em macOS/Windows/Linux e não há diálogos de autorização.
- **Trade-off:** é criptografia *em repouso*. Protege contra o `app.db` ser
  copiado/sincronizado **sem** o `vault.key`, mas não contra um atacante local com acesso
  aos dois arquivos. (Proteção forte exigiria uma master password — não implementada.)
- **Migração do Keychain antigo:** instalações anteriores guardavam a senha no cofre do SO
  via crate `keyring` ([storage/secrets.rs](src-tauri/src/storage/secrets.rs), agora
  só-migração). Na 1ª conexão de cada servidor, a senha legada é lida do Keychain (um
  último prompt do SO), re-criptografada no vault e removida do Keychain.
- **Migração da URI legada:** URIs gravadas em texto puro por versões anteriores são
  cifradas em lugar na primeira leitura, sem intervenção do usuário. Como o SQLite não
  sobrescreve páginas liberadas, a migração marca `pending_vacuum` na tabela `meta` e o
  boot seguinte roda um `VACUUM` para levar o resíduo em texto puro junto.
- **Canário** ([storage/health.rs](src-tauri/src/storage/health.rs)): um valor conhecido,
  cifrado com a mesma chave e guardado em `meta`. No boot ele responde "a chave em
  `vault.key` ainda abre o que está guardado?". Se não abrir, o front avisa
  (`vault_status`) em vez de deixar toda senha decifrar para vazio e o banco responder
  *authentication failed* — que era o sintoma enganoso de antes. No caminho de conexão,
  um envelope que não abre agora falha com `VAULT_UNAVAILABLE`; a lista de servidores
  continua abrindo, com a URI opaca.
- **Segredo não sobrevive ao `drop`:** `Server`/`ServerInput` implementam `Drop` com
  `zeroize` (o Rust libera a `String` sem apagar os bytes) e têm `Debug` escrito à mão,
  que imprime `<redigido>` no lugar da senha e da URI. Cópias dentro dos pools
  (deadpool/mongodb/redis) seguem fora do nosso alcance.
- **Leitura sob demanda:** a senha só é decifrada ao **criar** um pool (cache-miss);
  comandos sobre uma conexão já aberta não tocam no cofre.

### Estado: servidor vs. UI
- **React Query** (`src/queries/`) é a fonte da verdade dos dados do backend (servidores,
  estrutura, dados de tabela). Domínios de metadados são **persistidos em IndexedDB**
  ([providers/query-provider.tsx](src/providers/query-provider.tsx)); dados de tabela são
  sempre ao vivo.
- **Zustand** (`src/stores/`) guarda só estado de UI: abas abertas, nós expandidos da
  árvore, tabelas recentes, tema. Mutações invalidam as query keys relevantes
  ([queries/keys.ts](src/queries/keys.ts)) para disparar refetch.

## Fluxo típico (abrir uma tabela)

1. Usuário expande um servidor → `list_databases` → `connect_adapter` cria o pool
   (lê a senha do cofre **uma vez**).
2. Expande um database → `list_schemas_with_tables` (cache hit, sem cofre).
3. Clica numa tabela → abre aba de browse → `fetch_table_data` monta o SELECT/find/scan
   no backend e pagina, devolvendo um `QueryResult`. Postgres aceita `whereExpr`.
4. Ordenar/filtrar/paginar → reenvia `fetch_table_data` com novo `TableDataRequest`.
5. Editar/inserir/remover linhas → alterações ficam pendentes no front e são aplicadas
   em lote no salvar (`apply_row_edits` / `insert_rows` / `delete_rows`), seguido de
   refetch via invalidação.
