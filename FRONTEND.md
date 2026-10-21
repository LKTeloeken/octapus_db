# Frontend React — Guia de Arquitetura

Referência do frontend do **octapus_db** (React 19 + TypeScript + Vite, dentro do Tauri).
Complementa o [BACKEND.md](BACKEND.md): aqui está como o front se organiza, gerencia estado
e fala com o backend.

---

## 1. Camadas

```
features/    telas e sua lógica (hooks use-*) — orquestram queries + stores
components/  apresentação reutilizável: results-table, query-editor, ui (Radix/shadcn)
queries/     hooks React Query — ÚNICA fonte de dados do backend
stores/      Zustand — estado de UI (abas, árvore, recentes, tema)
api/         camada tipada de invoke (client + um módulo por domínio)
providers/   QueryProvider (React Query + persistência IndexedDB)
lib/ shared/ utilidades puras e hooks genéricos
```

**Regra de ouro:** o fluxo é `feature → query/store → api`. Componentes nunca chamam
`invoke` direto, e a lógica de uma tela vive no seu hook `use-*` (o `.tsx` é só render).

As dependências só descem: `api/` não importa nenhuma camada, e `queries/`/`stores/` não
importam de `features/` nem de `components/`. Store Zustand mora sempre em `src/stores/` —
mesmo o que só uma feature usa hoje, porque logo outra camada precisa dele (ex.: excluir
um servidor limpa o `query-results-store`).

---

## 2. Camada de API (`src/api/`)

Cada comando do backend tem uma função tipada. O wrapper `call`
([client.ts](src/api/client.ts)) chama `invoke` e converte a rejeição (`{ code, message }`) em `ApiError`, com `code`
(`UNKNOWN` quando a rejeição não veio do app — argumento inválido, plugin). Para decidir
pelo tipo do erro, use `hasErrorCode(error, 'CANCELLED')`, nunca o texto da mensagem.

```ts
// src/api/commands.ts — nomes dos comandos
export enum RustCommand { ExecuteQuery = 'execute_query', /* … */ }

// src/api/query.ts — funções por domínio
export function executeQuery(serverId, database, query, options?) {
  return call<QueryResult>(RustCommand.ExecuteQuery, { serverId, database, query, options });
}
```

Módulos: `servers.ts`, `connection.ts`, `structure.ts`, `catalog.ts`, `browse.ts`,
`query.ts`, `export.ts`, `session.ts`, `window.ts`. Tipos em `src/api/types/` (`server`,
`structure`, `catalog`, `browse`, `query`, `capabilities`): os do IPC são **gerados do
Rust** pelo ts-rs em `src/api/types/generated/` (`pnpm gen:types`; também a cada
`cargo test`) e os `*.types.ts` só os reexportam, mais o que é só do front (constantes,
`ApiError`, `CatalogPath` com nome do front). O `src/api/commands.test.ts` confere que
`generate_handler!`, `RustCommand` e os handlers do mock têm os mesmos comandos. O `catalog.ts` também expõe
`onCatalogEvent` — o `listen` do evento `catalog-event` (ver §4).
Os formatos seguem o BACKEND.md §3 — tudo em `camelCase`, célula sempre `string | null`.

---

## 3. Estado: React Query (servidor) + Zustand (UI)

### React Query — `src/queries/`
Fonte da verdade de **tudo que vem do backend**. Um hook por domínio:

| Hook | Comando |
|---|---|
| `use-servers` | CRUD de servidores |
| `use-capabilities` | `get_capabilities`; `useDbCapabilities` → `get_db_capabilities` (tabela por tipo) |
| `use-databases` / `use-columns` / `use-indexes` | databases, colunas e índices (sob demanda) |
| `use-catalog` | catálogo do backend: `catalogStatusQuery` (abre com `catalog_open`), `catalogChildrenQuery`, `catalogShapesQuery`, `useCatalogSearch`, `useCatalogRelationSize`, `useCatalogRefresh`, `useCatalogDiagnostics`, `catalogCompleteQuery` (completar por prefixo: menções do assistente), `useSchemaComparison` (dois schemas, key `schema-comparison` fora do prefixo do catálogo); `hasCatalog`/`isFlatCatalog` dizem quem usa |
| `use-catalog-events` | liga o `catalog-event` às invalidações (montado pela árvore) |
| `use-refresh-structure` | "Atualizar" da árvore (servidor, database, schema, tabela) |
| `use-structure` | estrutura inteira — só Redis (os demais usam o catálogo) |
| `use-table-data` | `fetch_table_data` (browse, com `infiniteQuery`) |
| `use-execute-query` | editor livre |
| `use-apply-row-edits` | `save_row_changes` (tudo numa transação) e `apply_row_edits` / `insert_rows` / `delete_rows` |
| `use-write-export-file` | `write_export_file` (exportação da grade) |
| `use-structure-change` | `preview_structure_change` (prévia do SQL) e `apply_structure_change` (DDL da árvore) |
| `use-assistant-status` | `assistant_status` (configurado, online, logado) |
| `use-assistant-session` | `assistant_redeem` / `assistant_logout` |
| `use-assistant-me` | `assistant_me`; `consentPending` e `quotaRemaining` |
| `use-assistant-consent` | `assistant_consent_current` e `assistant_consent_accept` (recarrega o termo no `ASSISTANT_CONFLICT`) |
| `use-assistant-models` | `assistant_models` (só no modo `free`) |
| `use-assistant-feedback` | avaliação do turno (`assistant_feedback_set`/`_clear` numa mutação só; `null` remove): otimista na conversa do cache, volta ao estado anterior se o servidor recusar; as mudanças do mesmo turno saem em fila (`scope`) e a conversa só é relida quando a fila esvazia |
| `use-assistant-conversations` | conversas (`assistant_conversations`, `assistant_conversation`, `assistant_conversation_create`), o turno (`useAskAssistant`, com o `onEvent` do canal; ao terminar relê a conversa e a cota), `useCancelAssistantTurn` e a execução manual (`useExecuteAssistantQuery`) |

- **Query keys** centralizadas em [queries/keys.ts](src/queries/keys.ts). Mutações
  invalidam a key da tabela afetada → refetch automático.
- **Persistência:** domínios de metadados (`servers`, `capabilities`, `databases`,
  `structure`, `columns`, `indexes`) vão para IndexedDB; dados de tabela são sempre ao
  vivo, e o catálogo também não vai (o backend já guarda em disco, cifrado). Config em
  [providers/query-provider.tsx](src/providers/query-provider.tsx)
  (`refetchOnWindowFocus: false`, `retry: 1`). O `buster` sobe quando o formato do cache
  persistido muda (`catalog-v1` descartou a estrutura inteira que os Postgres guardavam).
- **Assistente:** keys sob `['assistant', …]`, **fora** do `PERSISTED_DOMAINS` — nada do
  assistente vai para o disco (RN-01; `queries/assistant-keys.test.ts`). O que depende do
  login fica sob `['assistant', 'session']`: um `ASSISTANT_UNAUTHORIZED` (o backend já
  apagou o token) remove esse prefixo e refaz o status (`queries/assistant-errors.ts`).
  Só `ASSISTANT_UNAVAILABLE` ganha nova tentativa. As mutações do assistente também
  ficam sob `['assistant', …]`: o `shouldPersistMutation` do `query-provider.tsx` não
  grava as pausadas (o comentário de uma avaliação na fila, por exemplo).
- **Prova do disco (B10, critério 8):** o `providers/persisted-cache.test.ts` passa um cache
  com conversa, lista, participante e avaliação pausada (com canário) pelas mesmas
  `PERSIST_DEHYDRATE_OPTIONS` do provider e confere que nada do assistente sai no que vai
  ao IndexedDB; o `stores/tabs-session.test.ts` enche o `assistant-store` e confere que o
  snapshot das abas não leva nada dele. O lado do Rust (log, `app.db`) e os canários na
  API estão no BACKEND.md, seção do assistente.

### Zustand — `src/stores/`
Só estado de **UI**, nada que o backend possa fornecer:

| Store | Responsabilidade |
|---|---|
| `tabs-store` | abas abertas (query e browse) e aba ativa — persistidas como sessão |
| `query-results-store` | resultado, log de mensagens e aba inferior de cada aba de query (efêmero, por id de aba) |
| `tree-store` | nós expandidos da sidebar, filtro e tamanho da janela de cada nó grande, databases agrupados por formato |
| `recent-tables-store` | tabelas abertas recentemente (command palette) |
| `connection-store` | registro best-effort de quais `(server, db)` já conectaram na sessão |
| `focus-store` | pedido de foco do teclado entre a árvore e a grade |
| `ui-store` | tema e flags de UI |
| `command-palette-store` | paleta de comandos aberta/fechada — aberta pelo `Cmd/Ctrl+K`, pela busca da sidebar e pelo botão Comandos — e o schema fixado nela (em memória, sobrevive ao fechar) |
| `structure-editor-store` | diálogo de estrutura aberto pelo menu da árvore (criar/renomear/excluir tabela, nova/editar/excluir coluna, comparar dois schemas) |
| `value-panel-store` | painel de valor da aba de tabela: aberto/fechado e preferências de formatação (global, persistido) |
| `assistant-store` | painel do assistente: aberto/fechado, abas de conversa (alvo, id da conversa, modelo, rascunho), turno ao vivo de cada aba e resultados por tentativa — **só em memória**, fora do `tabs-store` (RN-01) |

**Sessão das abas** ([stores/tabs-session.ts](src/stores/tabs-session.ts)): as abas
sobrevivem ao fechamento e ao update do app. O `main.tsx` chama `restoreTabsSession()`
antes do primeiro render; daí em diante cada mudança do `tabs-store` é gravada via
`save_session` (SQLite do backend, não `localStorage`) com throttle de 500 ms. Entra só a
intenção da aba — query escrita, ordenação, filtro, colunas ocultas; resultados e
edições pendentes não. Antes de sair por conta própria (instalar/reiniciar o update),
chame `flushTabsSession()`. Campo novo em `QueryTab`/`BrowseTab` ⇒ trate-o no
`parseTab` (com default, para snapshots antigos continuarem válidos).

### O catálogo no front
Postgres, Mongo e SQLite não têm a estrutura no front: ela mora no catálogo do backend
(BACKEND.md §1.1), e as telas pedem **fatias**.

- **Keys fracionadas** sob `['catalog', serverId, database, …]` (status, cada janela de
  filhos com caminho + filtro + limite, grupos por formato, tamanho, resolve, complete) e
  `['catalog-search', …]` para a palette. `staleTime: Infinity`: nada fica velho sozinho.
- **Eventos → invalidação** ([use-catalog-events.ts](src/queries/use-catalog-events.ts),
  função pura `catalogEventInvalidations`): `syncing`/`error`/`cancelled` → status;
  `schemas`/`relations` → as fatias daquele database e a busca; `ready` → também marca
  como velhas (sem refetch) as colunas dos schemas que mudaram.
- **Sem schema:** Mongo e SQLite guardam tudo no schema sem nome `FLAT_SCHEMA` (`''`);
  pede-se `{ kind: 'schema', schema: '' }` e, na aba e na palette, ele vira `null`.
- **Janelas:** listas grandes vêm de 500 em 500 (`CATALOG_PAGE_SIZE`); filtro e "carregar
  mais" mudam a key, e `keepPreviousData` segura a janela anterior na tela.
- **Servidor editado ou excluído:** as keys do servidor são invalidadas/removidas
  (`invalidateServerScope`), e o backend esquece os catálogos dele.

---

## 4. Telas (`src/features/`)

Shell em [app.tsx](src/app.tsx): `Sidebar` (esquerda) + `QueryTabs` (direita) + command
palette, dentro do `QueryProvider`.

- **`sidebar` / `connection-tree`** — árvore virtualizada de servidores → bancos →
  schemas → tabelas → colunas, montada como lista plana em `use-connection-tree`
  (`FlatRow`: nó, erro, filtro, "carregar mais"). Com catálogo, cada nível é uma janela de
  500 com **linha de filtro** acima de 50 itens (`/` foca o filtro; Enter/↓ voltam à
  lista; Esc limpa) e **"carregar mais"**; a lista de databases de um servidor ganha o
  mesmo filtro e janela, aplicados no front. Mongo e SQLite mostram as relações direto sob
  o database. Num database multi-tenant:
  - **agrupar por formato** (ação do database, estado em `tree-store.shapeGrouped`):
    "Formato principal", "Variação" (aviso `−3` com as tabelas que faltam no `title`) e
    "Outros schemas"; o nó do grupo usa `NodeRef.shape`, os schemas de dentro mantêm o
    id de sempre;
  - **aviso de drift** na lista simples, no lugar da contagem de tabelas;
  - **"Copiar diagnóstico"** (contagens e tempos, sem nomes —
    [lib/catalog-diagnostics.ts](src/lib/catalog-diagnostics.ts)).

  Tamanho só da tabela aberta (`catalog_relation_size`). Rótulos dos grupos e do aviso em
  [shape-labels.ts](src/features/connection-tree/shape-labels.ts). O Redis segue com a
  estrutura inteira (`use-structure`).
- **`structure-editor`** — DDL pela árvore (Postgres e SQLite), aberto pelo menu dos nós
  (`structure-editor-store`, montado na sidebar): **schema** (ou o database, no SQLite) →
  "Nova tabela"; **tabela** → "Nova coluna", "Renomear", "Excluir tabela/view" (CASCADE
  opcional no Postgres); **coluna** (clique direito) → "Editar coluna", "Excluir coluna".
  Todo diálogo mostra o SQL que o backend vai rodar (`SqlPreview`, prévia com debounce de
  250 ms — erro de validação aparece ali) e só aplica com a prévia em dia. Depois de
  aplicar, o backend recarrega o schema no catálogo (o `catalog-event` atualiza a árvore);
  renomear/excluir fecha a aba da tabela. No SQLite, mudar tipo/nulo/default avisa que a
  tabela é recriada.
- **`schema-compare`** — "Comparar schemas" no menu de um database ou schema Postgres
  (num schema, ele já vem como base): escolhe a base e o outro (sugestões do catálogo,
  filtradas pelo que se digita), e o modal lista as tabelas que só um tem e, nas comuns,
  as colunas que diferem — só na base, só no outro ou com tipo, nulo ou default
  diferente. Rótulos em [change-labels.ts](src/features/schema-compare/change-labels.ts).
- **`backup`** — backup e restauração do Postgres pelas ferramentas nativas. Menu da
  árvore: "Fazer backup…" em database, schema e tabela; "Restaurar backup…" no database
  (`backup-dialog-store`, montado no shell por `BackupDialogs`). O diálogo de backup
  escolhe formato (`.dump`/`.sql`), conteúdo e opções — `normalizeBackupOptions` desliga
  o que não combina — e mostra a linha de comando do backend (`CommandPreview`). O de
  restauração lê o arquivo (`restore_inspect`: formato e resumo do `.dump`), oferece
  "Ignorar dono", "Apagar objetos existentes" (pede confirmação) e "Tudo ou nada". Os
  dois checam as ferramentas (`usePgToolsStatus`). Ao iniciar, o diálogo fecha e o job
  vira um card no canto (`BackupJobs` → um `toast.custom` por job, lendo o
  `backup-jobs-store`, alimentado pelo `backup-event` em `useBackupEvents`): log,
  cancelar, "Mostrar na pasta", erros ignorados. Fim de restauração invalida o cache do
  database (`invalidateDatabaseScope`).
- **`server-form`** — criar/editar servidor. A senha **nunca** volta do backend: no modo
  edição o campo começa vazio e deve ser redigitado. **Escopo** opcional: "Databases
  visíveis" e, no Postgres, "Schemas visíveis" (padrões com `*`, `?` e `!` para excluir —
  aplicados no backend).
- **`settings`** — diálogo de configurações (engrenagem no rodapé da sidebar): menu de
  seções à esquerda, a ativa à direita. Seções num registro
  ([settings-sections.ts](src/features/settings/settings-sections.ts)); cada configuração
  é uma `SettingRow`. Os valores vêm de `useSettings`/`useUpdateSettings` (SQLite do
  backend, `get_settings`/`update_settings`) — menos o tema, que segue no `ui-store`.
  A preferência **versões beta** muda o canal do `update-notifier` (`checkForUpdate` em
  `src/api/updater.ts`); ligá-la checa na hora. A seção **PostgreSQL** mostra o
  `pg_dump` em uso (`usePgToolsDetection`) e deixa escolher a pasta ou voltar à detecção
  automática (`useSetPgToolsDir`, que refaz a busca).
- **`query-tabs`** — gerencia abas; cada aba é um editor livre (`query-editor`) ou um
  browse (`table-browser`).
- **`query-editor`** — editor CodeMirror + execução; `use-query-runner` roda a query,
  pagina e aplica edições. Resultados ficam no `stores/query-results-store`.
  **Autocomplete SQL** ([use-sql-completion.ts](src/features/query-editor/use-sql-completion.ts)
  + [sql-completion/](src/components/query-editor/query-editor/sql-completion)): com
  catálogo, o namespace compilado é só a estrutura **quente** — o schema da aba, o
  `public`, os schemas citados no statement e o de até 8 tabelas sem schema
  (`catalog_resolve`), trazidos pela porta `warm`; os demais schemas entram por prefixo
  (`completeSchemas`, a partir de 2 letras). Colunas ainda não carregadas de um tenant saem
  na hora emprestadas da mesma tabela de outro tenant em cache (`peekSimilarColumns`,
  marcadas "prévia de …") e as reais chegam em segundo plano. No SQLite o banco (`main`)
  é o schema padrão.
- **`table-browser`** — navegação de tabela; `use-table-browser` traduz cliques de
  ordenar/filtrar em `TableDataRequest` e orquestra o salvar (edits + inserts + deletes).
  É dono do painel de valor (só existe aqui, não no editor livre): botão **Valor** ao lado
  do WHERE e atalho `Cmd/Ctrl+I` (ou `F7`, o do DBeaver), registrado em fase de captura
  para funcionar também de dentro do CodeMirror do painel.
- **`command-palette`** — `Cmd/Ctrl+K` (ou a busca da sidebar / botão Comandos, via
  `command-palette-store`). Nos bancos com catálogo a busca é do backend
  (`catalog_search`, 50 resultados, nos catálogos abertos) e a mesma tabela em vários
  schemas vira **um grupo** ("orders em 5.000 schemas"): Enter reescreve a busca para
  `.orders` com o cursor antes do ponto, para escolher o schema. **Tab fixa um schema**
  (o do item ativo, se for um schema; senão o trecho antes do ponto ou o texto todo —
  ver `resolvePin`) e a busca passa a ser só nas relações dele (`catalog_search` com
  `schema`, nome exato); Shift+Tab ou Backspace no campo vazio solta. Abrir a paleta abre os
  catálogos dos databases das abas e dos recentes. O Redis segue no fuzzy local
  (`use-table-index`). Montagem dos itens em
  [palette-items.ts](src/features/command-palette/palette-items.ts).

---

- **`assistant`** — assistente de consulta (B7), num painel à direita das abas
  (`AssistantPanel`, montado no `app.tsx` quando `assistant-store.panelOpen`). Abre pelo
  botão "Assistente" da linha de abas, por `Cmd/Ctrl+J` (`use-toggle-assistant`: leva o
  banco da aba ativa) e por "Perguntar ao assistente" no menu de database/schema da árvore
  (C-01) — tudo só com `supportsAssistant` e num build com a API (`configured`).
  - **Portas** (`assistant-gate.ts`, puro): não configurado → atualização exigida → fora
    do ar (C-19) → login com convite → termo em Markdown (RN-11, `react-markdown` sem HTML,
    em chunk à parte) → pronto.
  - **Abas de conversa** próprias, em memória: `Cmd/Ctrl+T` nova e `Cmd/Ctrl+W` fecha
    **com o foco no painel** (listener de captura + `preventDefault`; o `use-query-tabs`
    ignora evento tratado; foco perdido no `body` vale a última zona clicada). Fechar
    cancela o turno. Histórico do servidor no relógio do cabeçalho (C-15).
  - **Alvo**: servidor (só os com `supportsAssistant`) e database; abrir o catálogo
    conecta (banco sem catálogo — o Redis — não abre nada: a estrutura sai de uma
    amostra das chaves no backend, a cada pergunta). Modelo só no modo `free`, antes da 1ª pergunta; cota no rodapé (RN-23).
  - **Menções `@`** (`mentions.ts`, `mention-completion.ts`): `@schema.tabela` no
    Postgres, `@tabela` no SQLite e `@coleção` no Mongo (só com `hasCatalog`: no Redis o
    `@` não completa e a tela não fala em citar), completadas por `catalog_complete` em fatias (nunca a
    estrutura inteira). Viram o `schemas` do turno: os citados na pergunta (o 1º vai no
    `search_path`), depois os das perguntas anteriores (relidos do histórico), depois o
    schema de onde a conversa abriu ou `public`.
  - **Turno** (`turn-reducer.ts` em `lib/assistant/`, puro): os eventos do canal viram o
    estado ao vivo, guardado por aba no store (trocar de aba não perde nada). O `delta` é o
    JSON parcial do `ModelOutput`; `partial-output.ts` extrai explicação, consulta e
    premissas enquanto chegam, e o `output` troca pela versão conferida. Só fica
    "executada" com o `result` ou o outcome registrado (RN-21).
  - **Resposta** (`turn-view.ts` + `turn-card.tsx`): consulta sempre visível com
    destaque (RN-18), premissas (RN-20), aviso de recorte (RN-26), aviso de código não
    executado (RN-14/19), revisão com a estimativa do plano, edição e Executar (RN-13),
    resultado paginado pelo `assistant_execute` na `ResultsTable` sem `editableInfo`
    (RN-16, C-09), "Abrir no editor" (C-10), tentativas que falharam (RN-29), limite
    esgotado com a última consulta (RN-24) e cancelar (C-18, também `Esc` no campo).
  - **Linguagem do banco** (`queryLanguage` das capabilities, nunca o `dbType`): no
    Mongo (B8) a consulta é a operação do contrato em JSON. `lib/assistant/query-text.ts`
    (puro) a indenta para leitura — é esse texto que vai à revisão e ao Executar —, a
    converte no comando do shell do editor ao "Abrir no editor" (`find` com ordem,
    salto ou limite vira `aggregate`, que o editor aceita) e dá os rótulos
    tabela/coleção. O destaque usa o dialeto `mongo`; a revisão mostra o caminho do
    plano (`Plano: IXSCAN …`), já que o Mongo não estima linhas. O código do Mongo vem
    como comando do shell e passa como está. No **Redis** (B9, `redis_plan`) a consulta é
    o plano de passos em JSON (indentado, destaque `mongo`), a revisão mostra os comandos
    (`Plano: SCAN → foreach HGET`) e os rótulos viram "padrão/padrões de chave". O editor
    do Redis roda um comando por vez, então "Abrir no editor" (`opensInEditor`) só vale
    para o plano de um passo sem `foreach` — vira a linha do `redis-cli`
    (`redisCommandLine`) — e para o código de uma linha; o resto desabilita o botão. O
    aviso de recorte, sem `@`, diz que os padrões vieram de uma amostra.
  - **Avaliação** (B11; `turn-feedback.tsx` + `use-turn-feedback.ts`, transições puras em
    `lib/assistant/feedback.ts`): abaixo de cada turno terminado com resposta na tela e
    já no histórico do servidor, 👍 e 👎 como botões de alternar (`aria-pressed`, rótulo
    "Ajudou"/"Não ajudou"). O mesmo botão de novo remove; o outro troca, levando o
    comentário. No 👎, os motivos em chips de alternar (ligado = `--iris-soft` + ✓) e o
    comentário aberto; no 👍, o comentário fica recolhido atrás de "Comentar". Comentário
    até 2000 caracteres (contador; acima disso o Salvar desliga) com o aviso "Não cole
    dados do seu banco aqui." no `aria-describedby`. Nota e motivos gravam no clique; o
    comentário, no Salvar. Tudo otimista (`use-assistant-feedback`); recusa do servidor
    volta ao estado anterior com um toast (`feedbackErrorMessage`).
  - **Grades várias na tela**: a `ResultsTable` com `keyboardScope` vira uma região de
    teclado (`results-table/keyboard-scope.ts`) e só atende atalhos com o foco nela; a
    grade da aba ignora o que nasce no painel do assistente.
  - O polvo animado (`components/octapus-animated`) marca o turno em andamento, com uma
    cena por fase (`turn-mascot.ts`) e, se o turno termina com o cartão aberto, uma cena
    de fim por um instante (`use-turn-mascot.ts`). O motor (rig, cenas e GSAP) fica em
    `octapus-animated/engine/` e só entra por `import()`; até carregar, aparece o polvo em
    CSS (`components/octapus-thinking`), que também é o da aba (14 px).
- **`settings`** ganhou a seção **Assistente**: execução automática (`assistantAutoExecute`)
  e o participante, com "Sair".

## 5. `ResultsTable` — o grid de dados

[components/results-table](src/components/results-table) é o componente central, usado
tanto pelo editor livre quanto pelo browse. Características:

- **Duas visualizações:** tabela (padrão) e vertical (registros como colunas), alternadas
  na status bar.
- **Virtualização** de linhas e colunas com TanStack Virtual (renderiza só o visível).
- **Estado de edição** isolado em [use-results-table.ts](src/components/results-table/use-results-table.ts):
  - edições de células (amarelo), linhas novas (verde) e linhas removidas (vermelho)
    ficam **pendentes** — nada é aplicado na hora;
  - seleção de linhas (clique no identificador, `Cmd`/`Shift` para múltipla) e de colunas
    (clique no header, visual);
  - atalhos: `Cmd/Ctrl+S` salva, `Backspace` remove as linhas selecionadas, `Esc` limpa
    a seleção;
  - ao **Salvar**, chama `onSave({ edits, inserts, deletes })`; o consumidor grava pelo
    [persistRowChanges](src/lib/persist-row-changes.ts) e a tela só atualiza após o `ok`
    (via invalidação/refetch). Com `supportsAtomicSave` vai tudo num `save_row_changes`
    (uma transação: se falha, nada vale e a grade mantém tudo pendente para reenviar);
    sem ela, `delete_rows` → `insert_rows` → `apply_row_edits`, cada etapa na sua
    transação;
  - as regras das pendências (desfazer ao voltar ao original, `''` ≠ NULL, linha nova só
    com as colunas tocadas, linha removida não leva a edição) são funções puras em
    [pending-changes.ts](src/components/results-table/pending-changes.ts), com teste. A
    linha existente é identificada pelos valores da PK em JSON — nada de juntar com
    separador, que confundia um valor com `|` ou o texto `"NULL"`.
- **Painel de valor** ([results-table-value-panel](src/components/results-table/results-table-value-panel)),
  ligado por `showValuePanel`: mostra e edita a célula sob o cursor (`focusedCell`), num
  painel redimensionável à direita. Colunas JSON, lista (array) e texto ganham o menu de
  formato (Binário/HTML/JSON/Texto/XML, quebra de linha, formatar automaticamente, salvar
  compactado, codificação); o resto é texto simples. A digitação vira edição **pendente**
  na hora (mesmo `updateCell` da grade) — não há "aplicar". JSON inválido numa coluna JSON
  não é aplicado; array do Postgres é editado como array JSON e volta como literal `{…}`.
  A formatação pura mora em [lib/value-format.ts](src/lib/value-format.ts) (JSON
  reindentado por tokens, sem `JSON.parse`, para não arredondar bigint).
- **Contrato com o consumidor:** props `columns`, `rows`, `editableInfo`, `onSave`,
  `onLoadMore`, `onReorderTable`. Editável só quando `editableInfo != null`.

---

### Erros de render e carregamento sob demanda
- [ErrorBoundary](src/components/error-boundary.tsx) em três níveis: o app inteiro (com
  "Recarregar o app"), a barra lateral e a aba ativa (`resetKeys` = id da aba: trocar de
  aba limpa o erro). O erro de render vira o estado de erro do DESIGN.md §8 e vai para o
  console; erro de evento ou Promise continua virando toast.
- Linguagens do CodeMirror pouco usadas (JavaScript do Mongo, XML/HTML do painel de valor)
  carregam sob demanda por [use-lazy-language](src/lib/use-lazy-language.ts): chunks à
  parte, e o editor abre na hora, sem destaque até o chunk chegar. SQL e JSON ficam no
  bundle principal. Biblioteca pesada nova que só uma tela usa entra do mesmo jeito.

---

## 6. Modo mock — front sem o Rust

`pnpm dev:mock` sobe só o Vite (porta **1430**) com um backend simulado, para trabalhar em
telas sem compilar o backend. O mock é instalado no **IPC** (`mockIPC` do
`@tauri-apps/api/mocks`), então `api/`, `queries/` e as features rodam idênticas ao
produção — inclusive o `Channel` de mensagens e o plugin de updater.

- Liga por `VITE_MOCK` (arquivo `.env.mock`); em `pnpm build` a variável não existe e o
  `import()` de `src/mocks/` é eliminado do bundle.
- Porta própria ⇒ origin própria ⇒ `localStorage` e IndexedDB isolados do app real.
- Badge **MOCK** no canto controla latência, injeção de erro, respostas vazias e reset
  dos dados — para desenhar loading, erro e empty state sem editar código.
- Dataset em `src/mocks/data/`: um servidor de cada tipo (Postgres com schemas/PKs/
  arrays, Mongo sem schemas, Redis só browsable), com tabelas grandes o bastante para
  exercitar a virtualização e a paginação de 500. Para escala: o database **`saas`** do
  Postgres (5.000 schemas de tenant × 150 tabelas, com drift nos múltiplos de 100) e
  2.000 databases `tenant_NNNN` no Mongo — nada é materializado de antemão.
- Catálogo simulado em `src/mocks/catalog.ts` (sincroniza em camadas emitindo
  `catalog-event`, carga prioritária, busca agrupada, grupos por formato, escopo,
  diagnóstico) e eventos globais em `src/mocks/events.ts`.
- API do assistente simulada em `src/mocks/assistant.ts`: o painel escolhe "no ar",
  "sem URL" (`ASSISTANT_NOT_CONFIGURED`) ou "fora" (`ASSISTANT_UNAVAILABLE`); entrar com
  um convite (8+ caracteres; `REVOG…` é recusado) deixa o termo pendente até o aceite.
  A avaliação (`src/mocks/assistant-turns.ts`) segue as regras da API; para ver o
  "desfazer" da tela, ligue "Falhar comando: próximo" no painel e clique em 👍/👎. No
  servidor Mongo ("Catálogo (Atlas)", database `catalogo`) os turnos devolvem a
  operação em JSON e a execução manual passa por um guard simplificado; no Redis
  ("Cache (dev)", database `0`), o plano de passos (`SCAN` com o total, `SCAN` +
  `foreach HMGET`), com o guard simplificado do plano.

**Regra de manutenção:** comando novo no backend ⇒ handler novo em
`src/mocks/handlers.ts`, na mesma lista do `RustCommand`. Sem handler o mock rejeita
dizendo qual comando falta. Detalhes e receitas em
[src/mocks/README.md](src/mocks/README.md).

---

## 7. Convenções

- **TypeScript** estrito; tipos de dados do backend em `src/api/types/`.
- **Tailwind v4** + componentes Radix no padrão shadcn (`src/components/ui/`); use o
  helper `cn` ([lib/utils.ts](src/lib/utils.ts)) para classes condicionais.
- **Imports** com alias `@/` para `src/`.
- **Padrão hook + view:** `feature.tsx` só renderiza; lógica em `use-feature.ts`.
- **Arquivos de tipos** colocados ao lado do componente como `*.types.ts`.
- **Erros do backend** chegam como `ApiError` (`message` + `code`) — trate com `toast`.
- **Lint:** `pnpm lint` roda o ESLint (`eslint.config.js`: TypeScript, regras dos hooks e
  `jsx-a11y`) e o verificador dos tokens do DESIGN.md (`scripts/check-design-tokens.mjs`).
  Erro reprova; os avisos que sobram são dívida: interação por clique em `div` na grade,
  na árvore, na paleta e nas abas (o teclado é tratado no contêiner, mas falta a
  estrutura ARIA de grid/tree/listbox). `no-autofocus` está desligado: só há autoFocus em
  campo que o usuário acabou de abrir. `pnpm format` / `pnpm format:check` (Prettier;
  os tipos gerados ficam de fora pelo `.prettierignore`).
- Rode `pnpm type-check` e `pnpm lint` antes de concluir mudanças no front, e `pnpm test` (Vitest) —
  hooks testados com `renderToString`: o zustand lê o estado **inicial** da store no SSR,
  então os testes preenchem `useTreeStore.getInitialState()`.
- Medições de desempenho do front em `src/perf/` (`pnpm perf:front`), ao lado das do
  backend (`perf/catalog/`).
