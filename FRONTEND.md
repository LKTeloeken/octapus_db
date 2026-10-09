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
([client.ts](src/api/client.ts)) chama `invoke` e converte a rejeição (string) em `ApiError`.

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
`structure`, `catalog`, `browse`, `query`, `capabilities`). O `catalog.ts` também expõe
`onCatalogEvent` — o `listen` do evento `catalog-event` (ver §4).
Os formatos seguem o BACKEND.md §3 — tudo em `camelCase`, célula sempre `string | null`.

---

## 3. Estado: React Query (servidor) + Zustand (UI)

### React Query — `src/queries/`
Fonte da verdade de **tudo que vem do backend**. Um hook por domínio:

| Hook | Comando |
|---|---|
| `use-servers` | CRUD de servidores |
| `use-capabilities` | `get_capabilities` |
| `use-databases` / `use-columns` / `use-indexes` | databases, colunas e índices (sob demanda) |
| `use-catalog` | catálogo do backend: `catalogStatusQuery` (abre com `catalog_open`), `catalogChildrenQuery`, `catalogShapesQuery`, `useCatalogSearch`, `useCatalogRelationSize`, `useCatalogRefresh`, `useCatalogDiagnostics`; `hasCatalog`/`isFlatCatalog` dizem quem usa |
| `use-catalog-events` | liga o `catalog-event` às invalidações (montado pela árvore) |
| `use-refresh-structure` | "Atualizar" da árvore (servidor, database, schema, tabela) |
| `use-structure` | estrutura inteira — só Redis (os demais usam o catálogo) |
| `use-table-data` | `fetch_table_data` (browse, com `infiniteQuery`) |
| `use-execute-query` | editor livre |
| `use-apply-row-edits` | `apply_row_edits` / `insert_rows` / `delete_rows` |
| `use-write-export-file` | `write_export_file` (exportação da grade) |

- **Query keys** centralizadas em [queries/keys.ts](src/queries/keys.ts). Mutações
  invalidam a key da tabela afetada → refetch automático.
- **Persistência:** domínios de metadados (`servers`, `capabilities`, `databases`,
  `structure`, `columns`, `indexes`) vão para IndexedDB; dados de tabela são sempre ao
  vivo, e o catálogo também não vai (o backend já guarda em disco, cifrado). Config em
  [providers/query-provider.tsx](src/providers/query-provider.tsx)
  (`refetchOnWindowFocus: false`, `retry: 1`). O `buster` sobe quando o formato do cache
  persistido muda (`catalog-v1` descartou a estrutura inteira que os Postgres guardavam).

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
| `value-panel-store` | painel de valor da aba de tabela: aberto/fechado e preferências de formatação (global, persistido) |

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
  `src/api/updater.ts`); ligá-la checa na hora.
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
  - ao **Salvar**, chama `onSave({ edits, inserts, deletes })`; o consumidor dispara
    `delete_rows` → `insert_rows` → `apply_row_edits` e a tela só atualiza após o `ok`
    (via invalidação/refetch).
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
- **Erros do backend** chegam como `ApiError` (mensagem em string) — trate com `toast`.
- Rode `pnpm type-check` antes de concluir mudanças no front, e `pnpm test` (Vitest) —
  hooks testados com `renderToString`: o zustand lê o estado **inicial** da store no SSR,
  então os testes preenchem `useTreeStore.getInitialState()`.
- Medições de desempenho do front em `src/perf/` (`pnpm perf:front`), ao lado das do
  backend (`perf/catalog/`).
