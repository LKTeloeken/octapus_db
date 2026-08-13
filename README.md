# octapus_db

Cliente de banco de dados **desktop**, multi-banco, construído com **Tauri 2 + React 19**.
Conecta em **PostgreSQL, MongoDB e Redis** atrás de uma interface única: árvore de
estrutura, editor de queries, navegação de tabelas com paginação/ordenação/filtro
no servidor e edição inline de linhas — os três bancos expõem **os mesmos comandos**.

> Status: em desenvolvimento. MySQL e SQLite estão previstos (retornam "coming soon").

## Funcionalidades

- **Multi-banco** — Postgres, MongoDB e Redis com a mesma UI; o que muda por banco
  é decidido por `get_capabilities` (esconde nível schema, editor SQL, etc.).
- **Conexão lazy** — não há "abrir conexão"; qualquer comando conecta sob demanda e
  o pool fica em cache por `(servidor, database)`.
- **Sidebar em árvore** — bancos → schemas → tabelas/collections → colunas, carregados
  incrementalmente.
- **Editor de queries** — sintaxe nativa por banco (SQL / shell Mongo / comandos Redis),
  com CodeMirror, paginação e cancelamento.
- **Browse de tabela** — paginação, ordenação e filtros montados e validados no backend
  (sem SQL no front, sem injection).
- **Edição inline + inserção/remoção de linhas** — alterações ficam pendentes (verde =
  nova, vermelho = removida, amarelo = editada) e só são aplicadas ao salvar.
- **Command palette** (`Cmd/Ctrl+K`) para navegação rápida.
- **Senhas criptografadas** — cofre próprio (AES-256-GCM, chave presa ao dispositivo);
  sem prompts do SO e comportamento idêntico nos 3 sistemas.

## Stack

| Camada | Tecnologia |
|---|---|
| Shell desktop | Tauri 2 (Rust) |
| Backend | Rust — tokio, deadpool-postgres, mongodb, redis, rusqlite, keyring |
| Frontend | React 19 + TypeScript + Vite 6 |
| Estilo | Tailwind CSS v4 + Radix UI (padrão shadcn) |
| Estado do servidor | TanStack Query (React Query) v5 + persistência em IndexedDB |
| Estado de UI | Zustand |
| Virtualização | TanStack Virtual |
| Editor de código | CodeMirror 6 |

## Começando

### Pré-requisitos
- **Node** + **pnpm**
- **Rust** (toolchain estável; o projeto valida com 1.96)
- Dependências de sistema do Tauri 2 (WebKit). No **Linux**, `libdbus-1-dev` +
  `pkg-config` só são necessários para a migração única de senhas que ainda estejam no
  Secret Service de versões antigas — o uso normal não depende disso (ver
  [ARCHITECTURE.md](ARCHITECTURE.md#segredos--senhas)).

### Rodar
```bash
pnpm install
pnpm tauri dev     # app completo (Vite + Rust)
```

### Outros scripts
```bash
pnpm dev           # só o frontend (Vite) em http://localhost:1420
pnpm type-check    # tsc --noEmit
pnpm tauri build   # empacota o app de produção
# backend:
cd src-tauri && cargo build && cargo clippy && cargo test
```

## Estrutura do repositório

```
.
├── src/                 # Frontend React (ver FRONTEND.md)
│   ├── api/             # camada tipada de invoke (client + comandos por domínio)
│   ├── queries/         # hooks React Query (estado do servidor)
│   ├── stores/          # stores Zustand (estado de UI: abas, árvore, tema)
│   ├── features/        # telas: sidebar, connection-tree, query-editor, table-browser…
│   ├── components/      # results-table, query-editor e UI (Radix/shadcn)
│   └── providers/       # QueryProvider (React Query + persistência)
├── src-tauri/src/       # Backend Rust (ver BACKEND.md)
│   ├── commands/        # handlers #[tauri::command]
│   ├── adapters/        # DatabaseAdapter: postgres | mongo | redisdb
│   ├── services/        # ConnectionService (cache de pools), QueryService…
│   ├── storage/         # SQLite local + secrets (keychain)
│   └── models/          # tipos serializados para o front
└── .claude/             # instruções de trabalho para o agente (ver CLAUDE.md)
```

## Documentação

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — visão geral do sistema (front + back), fluxo
  de dados e decisões de design.
- **[BACKEND.md](BACKEND.md)** — referência do backend Rust: comandos `invoke`, modelos
  de dados e como cada tela consome o back.
- **[FRONTEND.md](FRONTEND.md)** — referência do frontend: estrutura, gerência de estado,
  camada de API e o componente de tabela.
