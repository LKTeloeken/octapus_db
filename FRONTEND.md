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

Módulos: `servers.ts`, `connection.ts`, `structure.ts`, `browse.ts`, `query.ts`.
Tipos em `src/api/types/` (`server`, `structure`, `browse`, `query`, `capabilities`).
Os formatos seguem o BACKEND.md §3 — tudo em `camelCase`, célula sempre `string | null`.

---

## 3. Estado: React Query (servidor) + Zustand (UI)

### React Query — `src/queries/`
Fonte da verdade de **tudo que vem do backend**. Um hook por domínio:

| Hook | Comando |
|---|---|
| `use-servers` | CRUD de servidores |
| `use-capabilities` | `get_capabilities` |
| `use-databases` / `use-structure` / `use-columns` / `use-indexes` | estrutura (lazy) |
| `use-table-data` | `fetch_table_data` (browse, com `infiniteQuery`) |
| `use-execute-query` | editor livre |
| `use-apply-row-edits` | `apply_row_edits` / `insert_rows` / `delete_rows` |

- **Query keys** centralizadas em [queries/keys.ts](src/queries/keys.ts). Mutações
  invalidam a key da tabela afetada → refetch automático.
- **Persistência:** domínios de metadados (`servers`, `capabilities`, `databases`,
  `structure`, `columns`, `indexes`) vão para IndexedDB; dados de tabela são sempre ao
  vivo. Config em [providers/query-provider.tsx](src/providers/query-provider.tsx)
  (`refetchOnWindowFocus: false`, `retry: 1`).

### Zustand — `src/stores/`
Só estado de **UI**, nada que o backend possa fornecer:

| Store | Responsabilidade |
|---|---|
| `tabs-store` | abas abertas (query e browse) e aba ativa |
| `tree-store` | nós expandidos da sidebar |
| `recent-tables-store` | tabelas abertas recentemente (command palette) |
| `connection-store` | registro best-effort de quais `(server, db)` já conectaram na sessão |
| `ui-store` | tema e flags de UI |

---

## 4. Telas (`src/features/`)

Shell em [app.tsx](src/app.tsx): `Sidebar` (esquerda) + `QueryTabs` (direita) + command
palette, dentro do `QueryProvider`.

- **`sidebar` / `connection-tree`** — árvore lazy de servidores → bancos → tabelas;
  carrega filhos ao expandir (`use-connection-tree`). Adapta níveis por `capabilities`
  (sem schema em Mongo/Redis).
- **`server-form`** — criar/editar servidor. A senha **nunca** volta do backend: no modo
  edição o campo começa vazio e deve ser redigitado.
- **`query-tabs`** — gerencia abas; cada aba é um editor livre (`query-editor`) ou um
  browse (`table-browser`).
- **`query-editor`** — editor CodeMirror + execução; `use-query-runner` roda a query,
  pagina e aplica edições. Resultados ficam no `query-results-store`.
- **`table-browser`** — navegação de tabela; `use-table-browser` traduz cliques de
  ordenar/filtrar em `TableDataRequest` e orquestra o salvar (edits + inserts + deletes).
- **`command-palette`** — `Cmd/Ctrl+K`; busca fuzzy de tabelas/servidores.

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
- **Contrato com o consumidor:** props `columns`, `rows`, `editableInfo`, `onSave`,
  `onLoadMore`, `onReorderTable`. Editável só quando `editableInfo != null`.

---

## 6. Convenções

- **TypeScript** estrito; tipos de dados do backend em `src/api/types/`.
- **Tailwind v4** + componentes Radix no padrão shadcn (`src/components/ui/`); use o
  helper `cn` ([lib/utils.ts](src/lib/utils.ts)) para classes condicionais.
- **Imports** com alias `@/` para `src/`.
- **Padrão hook + view:** `feature.tsx` só renderiza; lógica em `use-feature.ts`.
- **Arquivos de tipos** colocados ao lado do componente como `*.types.ts`.
- **Erros do backend** chegam como `ApiError` (mensagem em string) — trate com `toast`.
- Rode `pnpm type-check` antes de concluir mudanças no front.
