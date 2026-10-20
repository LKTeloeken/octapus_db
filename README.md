# octapus_db

Cliente de banco de dados **desktop**, multi-banco, construído com **Tauri 2 + Rust +
React 19**. Conecta em **PostgreSQL, MongoDB, SQLite e Redis** atrás de uma interface única:
árvore de estrutura, editor de queries, navegação de tabelas com paginação/ordenação/filtro
no servidor e edição inline de linhas.

Leve e rápido: sem JVM e sem Chromium embutido — usa o webview do sistema. O instalador tem
~9 MB no Windows e ~13 MB no macOS; o app abre a janela em ~0,35 s e fica em ~125 MB de
RAM parado (macOS, M3 Pro, somando os processos do WebKit).

> Status: em desenvolvimento ativo (1.1 em beta). MySQL está previsto.

## Funcionalidades

### Bancos e conexões
- **Multi-banco** — Postgres, MongoDB, SQLite e Redis com a mesma UI, cada um na sintaxe
  nativa (SQL, shell do Mongo, comandos Redis); o que muda por banco é decidido por
  `get_capabilities`.
- **Conexão lazy** — não há "abrir conexão"; qualquer comando conecta sob demanda e o pool
  fica em cache por `(servidor, database)`. SSL/TLS e URI de conexão.
- **Importar conexões** de DBeaver, pgAdmin, TablePlus, DataGrip, Beekeeper Studio ou de
  uma URL colada — com revisão antes de gravar.
- **Grupos de servidores** e **escopo** por servidor: databases e schemas visíveis por
  padrão (`*`, `?` e `!` para excluir).
- **Senhas criptografadas** — cofre próprio (AES-256-GCM, chave presa ao dispositivo); sem
  prompts do SO e comportamento idêntico nos 3 sistemas.
- **Sessão restaurada** — abas e contexto voltam ao reabrir o app.

### Bancos multi-tenant (milhares de schemas)
- **Catálogo de metadados no backend** — a estrutura é sincronizada em camadas e entregue
  em fatias; a árvore é virtualizada, com janela, **filtro** (`/`) e "carregar mais" —
  fluida com 5.000 schemas e 750 mil tabelas.
- **Agrupar por formato** — schemas com a mesma estrutura viram um grupo ("Formato
  principal", "Variação" com as tabelas que faltam, "Outros schemas").
- **Aviso de drift** — destaca o tenant cuja estrutura diverge dos demais.
- **Comparar schemas** — tabelas e colunas que só um tem, ou com tipo/nulo/default
  diferente.

### Consultar e editar
- **Command palette** (`Cmd/Ctrl+K`) — busca no catálogo; a mesma tabela em vários schemas
  vira um grupo ("orders em 5.000 schemas") e `Tab` fixa um schema.
- **Editor de queries** — CodeMirror com autocomplete SQL ciente do schema, paginação e
  cancelamento.
- **Browse de tabela** — paginação, ordenação e filtros montados e validados no backend
  (sem SQL no front, sem injection).
- **Grade virtualizada** (linhas e colunas), visão vertical e busca na grade.
- **Edição inline + inserção/remoção de linhas** — alterações ficam pendentes (verde =
  nova, vermelho = removida, amarelo = editada) e só são aplicadas ao salvar (`Cmd/Ctrl+S`).
- **Painel de valor** (`Cmd/Ctrl+I` ou `F7`) — ver e editar a célula como JSON, XML, HTML,
  texto ou binário.
- **Exportar** resultados em CSV, JSON ou SQL.

### Estrutura e manutenção
- **Editor de estrutura** (Postgres e SQLite) — criar, renomear e excluir tabelas e
  colunas pela árvore, sempre com prévia do SQL que vai rodar.
- **Backup e restauração do Postgres** — `pg_dump`/`pg_restore` com prévia do comando,
  progresso, log e cancelamento; detecta as ferramentas instaladas.

### App
- **Temas claro e escuro** (design system Ink, contraste AA).
- **Atualização automática**, com canal beta opcional.
- **macOS, Windows e Linux** (`.dmg`, `.exe`, `.deb`, `.rpm`, AppImage) e pacote Nix.

## Stack

| Camada | Tecnologia |
|---|---|
| Shell desktop | Tauri 2 (Rust) |
| Backend | Rust — tokio, deadpool-postgres, mongodb, redis, rusqlite, aes-gcm |
| Frontend | React 19 + TypeScript + Vite 6 |
| Estilo | Tailwind CSS v4 + Radix UI (padrão shadcn) |
| Estado do servidor | TanStack Query (React Query) v5 + persistência em IndexedDB |
| Estado de UI | Zustand |
| Virtualização | TanStack Virtual |
| Editor de código | CodeMirror 6 |

## NixOS

O pacote Nix mora na branch [`nix`](https://github.com/LKTeloeken/octapus_db/tree/nix),
só de empacotamento: ela reempacota o `.deb` oficial com `autoPatchelfHook` e é
atualizada sozinha a cada release publicada (workflow `nix-release.yml`).

```bash
nix run github:LKTeloeken/octapus_db/nix
```

Instalação no perfil, configuração declarativa e detalhes: ver o
[README da branch `nix`](https://github.com/LKTeloeken/octapus_db/tree/nix#readme).
O endereço antigo (`github:LKTeloeken/octapus_db`, sem `/nix`) ficou congelado na
1.0.0 e avisa sobre a mudança.

Para desenvolver, o flake da `main` traz o devShell com Rust, pnpm e as libs do WebKit —
`nix develop`, ou automático via `direnv` (o `.envrc` já está no repositório).

## Instalar

Baixe o instalador da sua plataforma nas
[releases](https://github.com/LKTeloeken/octapus_db/releases).

## Começando (desenvolvimento)

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

## Lançar uma versão

Cada versão é uma tag anotada `app-v<versão>` num commit da `main`. O push da tag
dispara o `release.yml`, que builda macOS (arm64 e x64), Windows e Linux exatamente
daquele commit e cria a release como **rascunho**. Antes de gastar os builds, o
workflow confere se a tag bate com a versão gravada no app.

```bash
git switch main && git pull
pnpm release:version 1.2.0     # package.json, tauri.conf.json, Cargo.toml e Cargo.lock
git commit -am "1.2.0"
git tag -a app-v1.2.0 -m "octapus-db 1.2.0"
git push origin main app-v1.2.0
```

Revise o rascunho no GitHub e publique. Uma versão estável vira a release "latest",
que o updater do app consulta. Uma versão com sufixo (`1.2.0-beta.1`) sai como
pre-release e só chega a quem ligou o canal beta. Para refazer o build de uma tag que
já existe, rode o workflow Release à mão e escolha a tag em "Use workflow from".

### Rollback

A tag de cada versão é o ponto de volta, mas o updater **só instala versões maiores**
do que a instalada. Por isso, voltar atrás é feito em duas frentes:

1. **Parar a distribuição.** Converta a release ruim em rascunho (Edit → Save as
   draft) ou marque a anterior como "Set as latest release". Quem ainda não atualizou
   deixa de receber a versão ruim. Apagar a release não apaga a tag.
2. **Corrigir quem já atualizou.** Lance uma versão nova a partir do código bom, com
   um número maior:

   ```bash
   git switch -c rollback/1.2.1 app-v1.1.0   # ou um git revert na main
   pnpm release:version 1.2.1
   git commit -am "1.2.1" && git tag -a app-v1.2.1 -m "octapus-db 1.2.1"
   git push origin rollback/1.2.1 app-v1.2.1
   ```

   Antes, confira se a versão ruim mudou algum formato local, porque o código antigo
   pode não ler o formato novo: as migrações do banco local (`storage/`), o
   `FORMAT_VERSION` do catálogo e o `buster` do cache do front. Nesse caso, leve a
   migração de volta junto com o rollback.

Os instaladores de cada versão continuam na página da release dela, para quem quiser
reinstalar à mão. A branch `nix` se atualiza quando a versão nova é publicada.

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
│   ├── adapters/        # DatabaseAdapter: postgres | mongo | sqlite | redisdb
│   ├── catalog/         # catálogo de metadados (Rust puro)
│   ├── services/        # ConnectionService, CatalogService, backup, importação…
│   ├── storage/         # SQLite local + cofre de senhas (vault)
│   └── models/          # tipos serializados para o front
├── nix/                 # pacote Nix congelado (o atual vive na branch `nix`)
└── .claude/             # instruções de trabalho para o agente (ver CLAUDE.md)
```

## Documentação

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — visão geral do sistema (front + back), fluxo
  de dados e decisões de design.
- **[BACKEND.md](BACKEND.md)** — referência do backend Rust: comandos `invoke`, modelos
  de dados e como cada tela consome o back.
- **[FRONTEND.md](FRONTEND.md)** — referência do frontend: estrutura, gerência de estado,
  camada de API e o componente de tabela.
- **[DESIGN.md](DESIGN.md)** — design system (Ink): cor, tipografia, espaço, movimento,
  vidro, componentes e o roteiro de migração visual.

## Licença

Licenciado sob **MIT** ou **Apache-2.0**, à sua escolha.

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

Salvo declaração em contrário, toda contribuição enviada intencionalmente para inclusão
neste projeto, nos termos da licença Apache-2.0, será licenciada como acima, sem termos
ou condições adicionais.
