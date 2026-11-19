# Plano — UI nativa do octapus-db em GPUI

> Etapa de planejamento. Nada foi alterado no repositório. Base da análise:
> `main` em `deebdf5` (1.1.1-beta.1) e `feat/core-cli` em `51865fb` (1.2.0-beta.1);
> DBFlux clonado (raso) em `59acb04` (17/11/2026), só lido.
>
> **Revisão 2 — decisões tomadas:** UI em repositório privado separado
> (`octapus-native`), núcleo por dependência git (opção B), migração por importação
> para o `octapus_db` no início da beta (M1). Sem CI automático no repositório
> privado. Sem assistente de IA; mascote na Fase 5. Fora do TCC. Windows tem
> usuários → plataforma de primeira classe.

---

## 1. Resumo executivo

**Decisão.** Novo app desktop em Rust, GPUI + `gpui-component`, num **repositório
privado próprio, `octapus-native`**, só com a UI. Ele consome o `octapus-core` por
**dependência git fixada por `rev`**, apontando para a branch **`feat/native-core`** do
`octapus_db` (criada a partir da `feat/core-cli`). No dia a dia, um `[patch]` local
aponta para o seu checkout. A `main` do `octapus_db` não é tocada; o app Tauri segue
sendo o produto até a Fase 6.

**Por que não depende de merge na `main`.** O núcleo só existe na `feat/core-cli`. A
`feat/native-core` nasce dela e recebe só mudanças aditivas no núcleo. Quando a
`feat/core-cli` puder ir para a `main`, a `feat/native-core` vai junto sem conflito.

**Boa notícia.** O núcleo já está quase pronto para uma UI nativa: os 88
`#[tauri::command]` são invólucros finos; eventos já saem por callbacks
(`EventSink`, `BackupSink`, `MessageSink`) sem Tauri. Faltam ~4 extrações pequenas e
**uma API nova de streaming de linhas** (hoje todo resultado é materializado em
`Vec<Vec<Option<String>>>`).

**Riscos principais**

| Risco | Gravidade | Mitigação curta |
|---|---|---|
| Grade virtualizada é 100% trabalho nosso (a web dava muito pronto) | Alta | Spike de 1M linhas na Fase 0, antes de investir no resto |
| GPUI/gpui-component pré-1.0, quebram a cada versão | Alta | Versão exata (`=`), upgrade só em janela planejada |
| Windows com usuários, mas sem CI automático | Média | Build manual do Windows em todo marco + teste numa máquina/VM Windows |
| Linux exige Vulkan (wgpu); Wayland/X11/NVIDIA variam | Média | Matriz de teste na Fase 0, incluindo VM com lavapipe |
| IME e teclas mortas (á, ç, ã) no editor | Média | Spike de digitação pt-BR na Fase 0, nos 3 SOs |
| Acessibilidade (leitor de tela) muito inferior à webview | Média | Limitação conhecida; cobertura total por teclado |
| Escopo: ~13 mil linhas de features no front (fora o assistente) | Alta | Paridade por prioridade (P0/P1/P2) |

**Esforço estimado** (semanas de dedicação integral, uma pessoa; meio período ≈ dobro)

| Fase | Conteúdo | Semanas |
|---|---|---|
| 0 | Repositório, esqueleto, build manual 3 SOs, 2 spikes (grade 1M, editor/IME) | 1,5–2 |
| 0b | Preparar o núcleo na `feat/native-core` (extrações aditivas + streaming) | 1–1,5 |
| 1 | Servidores + árvore do catálogo | 2–3 |
| 2 | Grade virtualizada + abrir tabela | 3–4 |
| 3 | Editor de query + execução + cancelamento + histórico | 3–4 |
| 4 | Edição inline, filtros, exportação | 2–3 |
| 5 | Paridade P1, atalhos, palette, sessão, mascote, polimento | 4–6 |
| 6 | Importação para o `octapus_db` (M1), empacotamento, beta, transição | 2 + 4 de beta |
| **Total** | | **~19–26 semanas** |

---

## 2. Diagnóstico do estado atual

### 2.1 Workspace, toolchain, dependências

- **Toolchain local:** rustc/cargo 1.96.0. Sem `rust-toolchain.toml` no repo; o CI de
  release usa `dtolnay/rust-toolchain@stable`. Núcleo e app em edition 2021.
- **`main`:** só `src-tauri/` (lock em `src-tauri/Cargo.lock`). A pasta
  `crates/octapus-core/` que aparece no disco é resto de outra branch (não rastreada).
- **`feat/core-cli`:** workspace na raiz, `members = ["src-tauri", "crates/*"]`, versão
  única em `[workspace.package]`, `[profile.dev]` enxuto (`line-tables-only`,
  `incremental = false`). `Cargo.lock` na raiz com **849 pacotes**.
- **Repositório `octapus_db` é público** → o repositório privado da UI baixa o núcleo
  por git sem token.
- **Dependências do núcleo:** tokio (full), tokio-postgres + deadpool-postgres,
  mongodb 3, rusqlite 0.31 (bundled), redis 0.27, rustls 0.23, aes-gcm, sqlparser 0.58,
  nucleo-matcher, postcard + lz4 (catálogo em disco), reqwest 0.13, log.
- **Release atual:** `release.yml` dispara só em tag `app-v*` e builda o Tauri; o pacote
  Nix baixa o binário da release. Um repositório separado não interfere em nada disso.

### 2.2 Comandos Tauri: o que é puro e o que é acoplado

88 comandos em 15 módulos (`src-tauri/src/commands/`, 2.143 linhas no total).

**Já são só delegação ao núcleo** (recebem `State<AppState>` e chamam o core):
servers (10), connections (5), browse (3), structure (8), catalog (16), session (2),
settings (2), import (4), export (1), cli (2), backup (7 de 8), queries (7 de 8).

**Acoplados ao Tauri — e o custo de cada um:**

| Comando / ponto | Acoplamento | Custo para a UI nova |
|---|---|---|
| `execute_query` | `Channel` + `Webview` para os notices | Trivial: a UI implementa `MessageSink` com um canal. O teto de 2.000 notices (`ChannelSink`) deve ir para o núcleo |
| `assistant_turn_ask` | `Channel` + ~130 linhas de orquestração no comando | **Fora do escopo** (sem assistente na UI nova) |
| `restore_start` | `AppHandle` só para pegar o state na task | Trivial |
| `open_log_dir`, `log_file_path` | `tauri-plugin-opener` | Reimplementar (crate `open` ou API do GPUI) |
| updater (canal beta + plugin) | `tauri-plugin-updater` | Reimplementar na Fase 6 |
| `lib.rs`: laço de manutenção | `tauri::async_runtime::spawn` (catálogo + `evict_idle` a cada 60 s) | Extrair para `octapus_core::maintenance::run(state)` |
| `lib.rs`: eventos `catalog-event`/`backup-event` | `app.emit` dentro de closures | Nenhum: o núcleo já recebe `Arc<dyn Fn(Event)>` |

**Lógica que mora no front TypeScript (precisa existir em Rust):** serialização da
exportação CSV/JSON/SQL (`components/export-dialog/serialize.ts`), formatação da cópia
da grade (`grid-clipboard`), contexto do autocompletar SQL (`sql-completion/`), atalhos
padrão (`src/lib/keybindings`), formato do snapshot de sessão, formatação/cor por tipo de
célula. A TUI já tem um autocompletar contextual em Rust puro
(`octapus-cli/src/tui/complete.rs`, 484 linhas, sem ratatui) — candidato a subir para o
núcleo e servir aos dois.

### 2.3 Estado da aplicação hoje

- **`AppState`** (`octapus-core/src/state.rs`): `storage: Mutex<rusqlite::Connection>`
  (app.db), `connections`, `queries`, `structure`, `catalog`, `backups`, `imports`,
  `assistant`. Abre com `AppState::open(data_dir)`.
- **Conexões/pool:** cache por `(server_id, database)` → `Arc<dyn DatabaseAdapter>`
  (Postgres com deadpool). `AppState::adapter()` reusa o pool e só decifra a senha no
  cache-miss. Pools ociosos há 10 min fecham (laço de manutenção).
- **Cancelamento:** o front gera `query_id` em `QueryOptions`; `cancel_query` só age se a
  conexão está em cache. Postgres: registro `query_id → PID` + cancelamento no servidor
  (cancela também o `COUNT`). SQLite: implementado. Mongo/Redis: no-op (default da trait).
- **Resultados:** **não há streaming de linhas.** `execute_query` embrulha o SQL em
  `SELECT * FROM (...) LIMIT n OFFSET m` (padrão 500) e devolve tudo materializado. Há
  `unlimited: true`, que materializa o resultado inteiro como `String`s. Só os
  *notices* chegam em streaming (`MessageSink`).
- **Catálogo:** em memória + disco, sincronização em segundo plano, consultas em fatias
  (`catalog_children`, `catalog_search`, `catalog_resolve`), eventos por callback. Já é o
  modelo certo para uma árvore nativa.
- **Multiprocesso:** app, CLI e TUI já dividem o mesmo `app.db` (migração `IMMEDIATE`,
  `user_version` = 1000). A UI nova entra como mais um processo, de graça.
- **Histórico de queries:** **não existe** (nem no núcleo, nem no front). É feature nova.

### 2.4 Bancos e abstração

PostgreSQL, MongoDB, SQLite, Redis. Trait `DatabaseAdapter` (`adapters/traits.rs`) com
defaults `UnsupportedType`, `capabilities()` → `AdapterCapabilities` (o front já decide
por capability, não por `dbType`). Catálogo por `CatalogSource` (Postgres, Mongo, SQLite);
Redis lista pelo adapter. Células sempre `Option<String>`.

### 2.5 Paridade com o front React (por prioridade)

**P0 — sem isso a UI nova não substitui a atual**
1. Servidores: criar/editar/testar, grupos, TLS, arquivo SQLite, senha nunca volta.
2. Árvore do catálogo: janelas + filtro, busca `Ctrl+P`, eventos de sincronização, Redis.
3. Abas de query: editor SQL/Mongo/Redis, rodar seleção/tudo, cancelar, log de mensagens.
4. Grade: virtualizada, cor por tipo, seleção, cópia, redimensionar/ordenar colunas,
   busca na grade, painel de valor, vista vertical (registro), seletor de colunas.
5. Abrir tabela: paginação/ordenação/filtro no servidor, contagem.
6. Edição inline com pendências (verde/vermelho/amarelo), editores por tipo (texto,
   número, booleano, data, JSON, UUID, array), salvar em transação.
7. Tema claro/escuro (Ink) e restauração das abas.

**P1 — necessário antes de virar a versão principal**
1. Command palette `Cmd/Ctrl+K` e atalhos configuráveis (com acordes `g g`).
2. Exportação CSV/JSON/SQL.
3. Editor de estrutura (DDL Postgres/SQLite) e visualizador de definição.
4. Preferências.
5. Backup/restauração Postgres (jobs com progresso).
6. Importar conexões (TablePlus, DataGrip, DBeaver, Beekeeper).
7. Atualizador + aviso de nova versão.
8. Mascote (polvo) — Fase 5, com a animação do GPUI.

**P2 — depois da transição**
1. Comparar schemas.
2. Instalar o CLI pelo app; abrir pasta de log.

**Fora do escopo da UI nova:** assistente de IA. Continua só no Tauri.

**Novo (não existe hoje, vale trazer do DBFlux):** histórico de queries, confirmação de
query perigosa (`DELETE` sem `WHERE`, `DROP`, `TRUNCATE` — o núcleo já tem
`assistant/guard/sql.rs` para classificar).

---

## 3. Arquitetura alvo

### 3.1 Repositórios e crates

```
~/Documents/projects/personal/
├── octapus_db/                    público. main intocada.
│   └── (branch feat/native-core, a partir da feat/core-cli)
│       └── crates/octapus-core/   núcleo: só adições
│
├── octapus_db-native-core/        git worktree do octapus_db na feat/native-core
│                                  (você pode trocar de branch no octapus_db sem
│                                   quebrar o build da UI)
│
└── octapus-native/                PRIVADO. Só a UI. Cargo.lock próprio.
    ├── Cargo.toml                 [workspace] members = ["crates/*"]
    ├── rust-toolchain.toml        1.96.0
    ├── .cargo/config.toml         [patch] → ../octapus_db-native-core (gitignored)
    ├── crates/
    │   ├── octapus-ui-kit/        tema Ink → Theme do gpui-component, tokens, ícones,
    │   │                          primitivas. Não conhece o núcleo.
    │   ├── octapus-grid/          grade virtualizada: RowStore, seleção, cópia,
    │   │                          pintura de células. Não conhece o núcleo.
    │   └── octapus-app/           binário: bridge tokio, estado, shell, documentos,
    │                              sidebar, editor, diálogos, keymap.
    ├── bench/                     seed e scripts do benchmark (§7)
    └── .github/workflows/
        └── build.yml              SÓ workflow_dispatch (manual)

Dependências (só descem):
  octapus-app ──► octapus-grid ──► octapus-ui-kit ──► gpui, gpui-component
       │
       └──────► octapus-core (git = octapus_db, rev = <sha da feat/native-core>)
```

Três crates, não sessenta: `grid` separada porque é a peça mais complexa, testável
sem o app e recompila sozinha; `ui-kit` separada para o tema não recompilar com a
lógica.

### 3.2 Dentro do `octapus-app`

```
src/
├── main.rs            abre Application, Root do gpui-component, janela
├── bridge/            runtime tokio + spawn de futures do núcleo + sinks → canais
├── model/             Entity<AppModel>: servidores, conexões abertas, catálogos, jobs
├── workspace/         shell: sidebar | documentos | painel inferior | status bar
├── documents/         enum Document { Query, Table, Structure, Definition, ... }
│   ├── query/         editor + resultados + log de mensagens
│   └── table/         grade + filtro + paginação
├── sidebar/           árvore (lê do catálogo em janelas)
├── actions/           keymap, palette, comandos
└── session/           snapshot das abas (arquivo próprio, ver §4.5)
```

### 3.3 Async: tokio × executor do GPUI

O DBFlux usa drivers **síncronos** rodando no `background_executor` do GPUI. Nosso
núcleo é **tokio assíncrono** — não vamos reescrevê-lo. Modelo:

1. Um `tokio::runtime::Runtime` multi-thread (2–4 workers) criado no `main`, guardado
   num `Global` do GPUI junto com `Arc<AppState>`.
2. Toda ida ao núcleo: `rt.spawn(future)` → o `JoinHandle` é aguardado dentro de
   `cx.spawn(async move |this, cx| { ... this.update(cx, ...) })`. O `JoinHandle` é só
   um future; funciona em qualquer executor.
3. A `Task` do GPUI fica num campo da entidade (cancela a espera quando a aba fecha).
   Cancelar a query **no banco** continua sendo `cancel_query(query_id)`; abortar a task
   tokio sozinho não para o servidor.
4. Eventos do núcleo (`EventSink` do catálogo, `BackupSink`, `MessageSink`) viram
   canais `futures::channel::mpsc`. Uma task de primeiro plano por canal drena
   **tudo o que estiver pendente** e faz um único `cx.notify()` — coalescência por frame.
5. Nunca bloquear a thread de UI: nada de `block_on` em handler.

### 3.4 Resultados em lotes até a grade virtualizada

**Núcleo (adição, sem quebrar nada):**

```rust
// adapters/traits.rs — método novo com default
async fn execute_query_streamed(
    &self,
    query: &str,
    options: QueryOptions,
    messages: Option<Arc<dyn MessageSink>>,
    rows: RowBatchSender,            // canal limitado (ex.: 8 lotes)
) -> Result<StreamSummary> {
    // default: roda o execute_query_with_messages de hoje e manda 1 lote só
}
```

- **Postgres:** `query_raw` → `RowStream`; acumula até 2.000 linhas **ou** 16 ms e envia
  um `RowBatch`. O primeiro lote leva as colunas.
- **SQLite:** `step` num `spawn_blocking`, mesmos lotes. **Mongo:** lotes do cursor.
  **Redis:** default (1 lote).
- **Backpressure:** o canal é limitado; se a UI não consome, o produtor espera e o
  stream do banco pausa. Teto configurável (padrão sugerido: 100 mil linhas, com
  "carregar tudo").
- **`RowBatch` compacto:** um `String` único por lote + `Vec<u32>` de offsets + bitmap
  de nulos. Mantém o contrato "célula = string ou null", mas 1M × 12 colunas cabe em
  ~150–250 MB, contra 600 MB+ com `Vec<Vec<Option<String>>>`.
  Fica fora de `models/` (não cruza IPC, não precisa de ts-rs).

**UI:**

```
tokio (adapter) ──RowBatch──► canal limitado ──► task de 1º plano (cx.spawn)
                                                   │ append no RowStore
                                                   │ notify no máx. 1×/frame
                                                   ▼
                               Entity<ResultSet>  { batches: Vec<Arc<RowBatch>>,
                                                    prefix: Vec<u32>,     // linha → lote
                                                    order: Option<Vec<u32>> } // ordenação
                                                   │
                     uniform_list(range visível) ──► formata só as ~40 linhas na tela
                                                     (texto truncado, SharedString em cache)
```

- "Abrir tabela" continua com paginação no servidor (`fetch_table_data`, 500 por
  página) — paridade com hoje. O streaming é para query livre.
- Ordenar o resultado carregado: permutação de índices calculada no
  `background_executor`, sem copiar células.
- Cabeçalho e corpo com rolagem horizontal sincronizada; largura das colunas medida na
  amostra do primeiro lote.

---

## 4. Isolamento e paralelismo

### 4.1 Os três lugares e o que muda em cada um

| Lugar | O que entra | O que nunca entra |
|---|---|---|
| `octapus_db` / `main` | Seus releases e hotfixes do Tauri, como hoje | Nada da UI nova |
| `octapus_db` / `feat/native-core` (a partir da `feat/core-cli`) | Só adições no `crates/octapus-core` | Código de UI; mudanças em `src-tauri/`, `src/`, CLI |
| `octapus-native` (privado) | Toda a UI | Cópia do núcleo |

### 4.2 Branch do núcleo + worktree

```bash
cd ~/Documents/projects/personal/octapus_db
git branch feat/native-core feat/core-cli
git push -u origin feat/native-core          # público: o rev precisa existir no GitHub
git worktree add ../octapus_db-native-core feat/native-core
```

O worktree deixa o `octapus_db` livre para você trocar de branch (hotfix na `main`,
trabalho na `feat/core-cli`) sem quebrar o build da UI.

**Sincronização (sempre merge, nunca rebase — a branch é publicada e longa):**

```
main ──(hotfix, seu fluxo atual)──► feat/core-cli ──(merge)──► feat/native-core
```

```bash
cd ~/Documents/projects/personal/octapus_db-native-core
git fetch origin && git merge --no-ff origin/feat/core-cli
```

Quando: logo depois de cada commit relevante na `feat/core-cli` e, no mínimo, uma vez
por semana. Como a `feat/native-core` só acrescenta no núcleo, o merge quase nunca
conflita; se conflitar, é no núcleo, e você resolve no mesmo dia.

### 4.3 Repositório privado da UI

```bash
cd ~/Documents/projects/personal
mkdir -p octapus-native/crates/{octapus-ui-kit,octapus-grid,octapus-app}/src
cd octapus-native && git init
printf '/target/\n/.cargo/config.toml\n' > .gitignore
gh repo create octapus-native --private --source . --remote origin
```

`Cargo.toml` (esboço):

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
octapus-core   = { git = "https://github.com/LKTeloeken/octapus_db", rev = "<sha da feat/native-core>" }
# Par conferido no DBFlux em 17/11/2026: gpui-pre =0.3.9 + gpui-component =0.7.3.
# Confirmar o par mais novo no dia; sempre versão exata.
gpui           = { package = "gpui-pre", version = "=0.3.9" }
gpui-component = { version = "=0.7.3" }
tokio          = { version = "1", features = ["rt-multi-thread", "sync", "time"] }
futures        = "0.3"

[profile.dev]                # mesmo racional do octapus_db (disco)
debug = "line-tables-only"
[profile.dev.package."*"]
debug = false
```

`.cargo/config.toml` (local, fora do git) — o build usa o seu worktree, não o `rev`:

```toml
[patch."https://github.com/LKTeloeken/octapus_db"]
octapus-core = { path = "../octapus_db-native-core/crates/octapus-core" }
```

**Atualizar o `rev`** só quando for gerar um build para alguém testar ou marcar um
ponto estável: push na `feat/native-core`, copiar o sha para o `Cargo.toml`, commit na
UI. No dia a dia o `[patch]` faz o núcleo local valer na hora.

### 4.4 Workflow manual (sem CI automático)

Nenhum build roda sozinho. Os testes são na sua máquina. Um único workflow, só manual,
para gerar binários quando for mandar para alguém testar (e para conferir o Windows):

```yaml
# .github/workflows/build.yml
on:
  workflow_dispatch:
    inputs:
      os:
        type: choice
        options: [all, windows, linux, macos]
```

- Matriz `windows-latest / ubuntu-latest / macos-latest` filtrada pelo input;
  `cargo build --release`, sobe o binário como artifact.
- Sem o `.cargo/config.toml` no CI → usa o `rev` do `Cargo.toml` (por isso o push do
  núcleo vem antes).
- **Custo:** repositório privado gasta minutos do Actions (plano gratuito: 2.000/mês;
  macOS conta 10×, Windows 2×). Rodar só o SO que precisa.
- **Artifact de repositório privado só é baixável por quem tem acesso ao repositório.**
  Para um testador de fora: baixar e mandar o arquivo direto.
- Disparar: `gh workflow run build.yml -f os=windows`.

### 4.5 Regras para mudar o `octapus-core`

1. **Só adição:** módulo novo, função nova, método novo de trait **com default**. Nunca
   mudar assinatura de item `pub` usado por `src-tauri` ou `octapus-cli`.
2. Precisa mudar algo existente? Crie `foo_streamed`/`foo_v2` ao lado; o antigo só leva
   `#[deprecated]` quando o Tauri for aposentado.
3. **Feature flags** só se a adição trouxer dependência pesada (não é o caso previsto).
4. Tipos que não cruzam IPC ficam fora de `models/` → nada de `pnpm gen:types` à toa.
5. Todo commit na `feat/native-core` passa pelos dois lados: no worktree,
   `cargo build --workspace && cargo clippy --workspace --all-targets && cargo test --workspace`
   (Tauri e CLI continuam verdes) **e** `cargo check` no `octapus-native`.
6. **Estado da UI fora do `app.db` até a migração:** sessão e preferências da UI nova
   num arquivo próprio no diretório de dados (`native-ui.db` ou JSON). Motivo: subir o
   `SCHEMA_VERSION` numa branch paralela pode colidir com outra subida na
   `feat/core-cli`. A UI nova lê e grava servidores/conexões pelo núcleo normalmente,
   não toca em `workspace_session` nem regrava `app_settings` (mesma regra do terminal).
   Na Fase 6 esse estado pode ir para uma tabela do `app.db`.

**Extrações aditivas previstas (Fase 0b):**

| Extração | Para onde | Quem passa a usar |
|---|---|---|
| Laço de manutenção do `lib.rs` | `octapus_core::maintenance::run(Arc<AppState>)` | UI nova (o Tauri pode migrar depois) |
| Teto de notices do `ChannelSink` | `adapters::message_sink::CappedSink<S>` | UI nova |
| Streaming de linhas | `execute_query_streamed` + `RowBatch` | UI nova |
| Autocompletar contextual da TUI | `octapus_core::sql_complete` (cópia; o CLI migra depois) | UI nova e TUI |
| Serialização CSV/JSON/SQL | `services::export` (grava em stream no arquivo) | UI nova |
| Histórico de queries | arquivo próprio da UI (vira tabela do núcleo na Fase 6) | UI nova |

---

## 5. Fases

Mesma ordem que você sugeriu, com duas mudanças: (1) a Fase 0 ganha dois spikes —
a grade e o IME são os maiores riscos técnicos e precisam falhar cedo; (2) entra uma
Fase 0b só para o núcleo, para as fases de UI não pararem esperando backend. Também
troco "dock" por **shell fixo redimensionável** (sidebar | documentos | painel
inferior): o DBFlux não usa o `DockArea` do gpui-component, e arrastar painéis é escopo
extra.

**Regra para o Windows (tem usuários):** todo "pronto" de fase inclui rodar o build
manual do Windows e testar o que a fase entregou numa máquina ou VM Windows.

### Fase 0 — Repositório, esqueleto e spikes (1,5–2 semanas)
- Branch + worktree + repositório privado (§4.2–4.3).
- Janela com `gpui_component::Root`, tema Ink claro/escuro/sistema, shell vazio.
- `AppState::open(paths::data_dir())` + bridge tokio: listar os servidores salvos.
- **Spike grade:** 1M linhas sintéticas × 12 colunas em `uniform_list`; medir.
- **Spike editor:** `InputState` em modo código com highlight SQL; digitar
  `ação, pão, à, ü, ç` com ABNT2 e US-intl (teclas mortas).
- Workflow manual (§4.4) rodando uma vez nos 3 SOs.
- **Pronto quando:** compila e roda no seu Mac; build manual verde nos 3 SOs; spike de
  grade a 60 fps no Mac, num Linux Wayland e no Windows; acentos funcionam em macOS,
  Windows, GNOME Wayland e X11 (ou o problema está documentado com issue upstream).
- **Demo:** app abre em < 0,5 s, troca de tema, mostra suas conexões reais.

### Fase 0b — Núcleo pronto (1–1,5 semanas, pode intercalar)
- Extrações da tabela da §4.5 na `feat/native-core`.
- **Pronto quando:** testes novos no núcleo; Tauri e CLI da branch sem nenhuma mudança de
  comportamento; `cargo check` na UI nova verde.

### Fase 1 — Conexões + árvore (2–3 semanas)
- Formulário de servidor (TLS, SQLite com seletor de arquivo, grupos), testar conexão.
- Árvore do catálogo por janelas, filtro por nó, "carregar mais", eventos de sync,
  Redis pelo adapter; busca `Ctrl+P` por `catalog_search`.
- **Pronto quando:** o database `saas` de teste (5.000 tenants) navega sem travar;
  criar/editar/apagar servidor reflete no Tauri aberto ao mesmo tempo (mesmo app.db).
- **Demo:** conectar num Postgres real e navegar 5.000 schemas.

### Fase 2 — Grade virtualizada (3–4 semanas)
- `octapus-grid`: seleção de célula/linha/coluna/retângulo, cópia (TSV + com cabeçalho),
  redimensionar coluna, ordenar, cor por tipo, painel de valor, vista vertical, busca na
  grade, seletor de colunas.
- Documento "abrir tabela" com paginação/ordenação no servidor.
- Streaming (`execute_query_streamed`) ligado à grade.
- **Pronto quando:** 1M linhas rolam a 60 fps (p95 < 16,7 ms); cópia de 100 mil
  células < 1 s; memória dentro da meta da §7.
- **Demo:** abrir uma tabela de 1M linhas e rolar até o fim.

### Fase 3 — Editor de query (3–4 semanas)
- Abas (dedup por chave: mesma tabela não abre duas vezes), highlight SQL/Mongo/Redis,
  autocompletar pelo catálogo (`CompletionProvider` do gpui-component +
  `sql_complete`), rodar tudo/seleção/statement no cursor, cancelar, log de mensagens em
  streaming, confirmação de query perigosa, histórico.
- **Pronto quando:** latência de digitação p95 < 8 ms num arquivo de 5.000 linhas;
  cancelar para a query no Postgres em < 1 s.
- **Demo:** escrever uma query com autocomplete, rodar, cancelar, reabrir do histórico.

### Fase 4 — Edição, filtros, exportação (2–3 semanas)
- Edição inline com pendências (cores do DESIGN.md §2.4), editores por tipo, inserir e
  remover linhas, salvar via `save_row_changes`, aviso de pendências ao fechar a aba.
- Filtro com autocomplete de `WHERE`; exportação CSV/JSON/SQL pelo `services::export`.
- **Pronto quando:** as mesmas edições dos e2e `grid-editing` funcionam à mão nos bancos
  que suportam.
- **Demo:** editar 3 linhas, apagar 1, inserir 1, salvar numa transação.

### Fase 5 — Paridade P1 e polimento (4–6 semanas)
- Command palette, keymap com contexts do GPUI e overrides do usuário, restauração de
  sessão, preferências, editor de estrutura, visualizador de definição, backup/restore,
  importar conexões, menus de contexto completos, estados vazios e de erro, toasts.
- **Mascote:** polvo com a animação do GPUI (sem GSAP; mapear as cenas para estados do
  app).
- **Pronto quando:** checklist P0 + P1 da §2.5 inteiro marcado; uma semana de uso
  diário seu sem voltar ao Tauri.
- **Demo:** fechar e reabrir o app com as abas, o texto e a rolagem de volta.

### Fase 6 — Migração M1, empacotamento e transição (2 semanas + 4 de beta)

**Pré-condição:** a `feat/core-cli` já está na `main` (aí a `feat/native-core` entra
junto — só tem adições). Se ainda não estiver, a beta pode começar com o núcleo pela
branch, mas a importação espera.

1. **Importar a UI para o `octapus_db` com histórico:**
   ```bash
   git clone <octapus-native> /tmp/native && cd /tmp/native
   git filter-repo --to-subdirectory-filter apps/octapus-native
   cd ~/Documents/projects/personal/octapus_db && git switch -c feat/import-native main
   git remote add native /tmp/native && git fetch native
   git merge --allow-unrelated-histories native/main
   ```
   Antes: conferir que o histórico privado não tem segredos (ele fica público).
2. **Ajustar:** dependência do núcleo volta a ser `path = "../../crates/octapus-core"`;
   `apps/octapus-native` com workspace e lock próprios (o glob `crates/*` da raiz não o
   pega); `.gitignore` do `target/` dela.
3. **Releases da beta saem do `octapus_db`:** tag `native-v*`, workflow próprio,
   bundles `.msi`/`.exe`, `.dmg` assinado/notarizado, AppImage/`.deb`; atualizador com
   `latest.json` próprio. Bundle id diferente durante a beta (instala lado a lado),
   **mesmo diretório de dados** (`octapus_core::paths`) → as conexões aparecem sozinhas.
4. **Usuários do Tauri:** a última versão Tauri mostra um aviso com o link do instalador
   novo (não usar o updater do Tauri para trocar de app). Tauri em modo só-correção por
   1–2 versões.
5. **Fim:** apagar `src-tauri/` e `src/` da `main`; arquivar o repositório privado.

- **Pronto quando:** zero bug P0 aberto por 2 semanas de beta, nos 3 SOs.

---

## 6. Riscos e mitigações

| Risco | Mitigação |
|---|---|
| **GPUI pré-1.0, quebras frequentes** | Versões exatas (`=`), upgrade numa janela de 1 dia, no máximo uma vez por mês, em commit isolado. API do GPUI usada fora de `ui-kit`/`grid` fica atrás de funções nossas |
| **Windows sem CI automático** | Build manual do Windows em todo fim de fase; teste numa máquina/VM Windows; spike de grade e de IME também no Windows |
| **Linux gráfico (Wayland/X11)** | O GPUI no Linux usa wgpu (Vulkan) e fala Wayland e X11 direto — sem WebKitGTK, mas com dependência de driver de GPU. Matriz na Fase 0: GNOME Wayland, KDE Wayland, X11 (tiling), NVIDIA proprietário, VM com lavapipe. No NixOS: `vulkan-loader`, `libxkbcommon`, `wayland` no wrapper (o flake do DBFlux mostra *como*, sem copiar) |
| **IME e acentos** | Spike na Fase 0 nos 3 SOs. Se falhar: issue upstream + teste manual a cada upgrade |
| **Acessibilidade** | Suporte a leitor de tela no GPUI é limitado. Limitação documentada na beta; 100% por teclado (teste de cobertura, como o DBFlux) |
| **Edição de célula, menus de contexto** | O gpui-component cobre `Input`, `NumberInput`, `DatePicker`, `Switch`, `Select`, `ContextMenu`, `Popover`. O resto (array, JSON em modal) é composição. Orçado nas Fases 2 e 4 |
| **Branch paralela do núcleo diverge** | Só adições; merge da `feat/core-cli` a cada mudança relevante e no mínimo semanal; build do Tauri/CLI verde no worktree a cada commit |
| **Estado da UI colidindo no app.db** | Arquivo próprio até a Fase 6 (§4.5, regra 6) |
| **Disco do notebook** | Núcleo compila também no `target/` da UI; GPUI em debug é pesado. `[profile.dev]` enxuto, `cargo clean` quando precisar |
| **Licenças** | gpui e gpui-component: Apache-2.0; octapus é MIT OR Apache-2.0 — compatível. Gramáticas tree-sitter: MIT. Fontes embutidas: OFL, exige aviso. `cargo deny check licenses` antes da beta, junto da pendência do THIRD_PARTY_LICENSES. Código do DBFlux: só referência |
| **Escopo infinito** | P0/P1/P2 fechados; assistente fora; P2 só depois da transição |

---

## 7. Testes e desempenho

### 7.1 Testes (todos locais)
- **Unidade (Rust puro):** `RowStore`, seleção, formatação de cópia, ordenação por
  permutação, keymap, `sql_complete`, `services::export`. Sem GPUI, rápidos.
- **Entidades GPUI:** `#[gpui::test]` com `TestAppContext` para fluxos de estado
  (abrir aba, dedup, pendências, cancelar).
- **Núcleo:** os testes atuais seguem valendo; streaming ganha teste e2e `#[ignore]` nos
  bancos do docker dos e2e.
- **Antes de mandar um build para alguém:** `cargo test && cargo clippy -- -D warnings`
  local + roteiro manual curto da fase + build manual do SO do testador.

### 7.2 Benchmark comparável Tauri × nativo
**Dados fixos:** Postgres em docker (porta própria, ex. 55433) com uma tabela de 1M
linhas × 12 colunas de tipos mistos (`generate_series`), mais o database `saas` de 5.000
schemas. Scripts em `octapus-native/bench/`.

**Instrumentação sem tocar no que vai para os usuários:** uma branch
`bench/tauri-marks` no `octapus_db`, nunca lançada, adiciona marcas de tempo no Tauri; o
nativo tem uma feature `bench` com as mesmas marcas.

| Métrica | Como medir | Meta nativa |
|---|---|---|
| Abertura até 1º frame interativo | `Instant` no `main` → marca no 1º frame pintado; 20 execuções, mediana | < 400 ms |
| Memória ociosa | Soma da árvore de processos (Tauri inclui os processos do WebKit/WebView2): `footprint` no macOS, PSS via `smaps_rollup` no Linux, Gerenciador de Tarefas/`Get-Process` no Windows | < 150 MB |
| Memória com resultado grande | 100 mil linhas abertas (o Tauri só chega a isso com `unlimited`) e 1M (só nativo) | 1M × 12 < 400 MB |
| Rolagem | 60 s de rolagem roteirizada; tempo de cada frame (rAF no Tauri, marca de frame no nativo); p50/p95/p99 e % acima de 16,7 ms | p95 < 16,7 ms com 1M |
| Latência de digitação | keydown → próximo frame apresentado (instrumentado) + Typometer para entrada→tela real | p95 < 8 ms |
| Primeira linha visível | "Rodar" → 1º lote pintado, query de 1M linhas | < 300 ms (streaming) |

Rodar no seu Mac, numa máquina Windows e numa Linux. Mesma query, mesmo banco, mesma
resolução de janela; resultado num CSV por versão.

---

## 8. Decisões

**Já tomadas**
1. Isolamento: repositório privado `octapus-native` + núcleo por git/`rev` (opção B).
2. Migração: importar para o `octapus_db` no início da beta (M1).
3. Sem CI automático; só workflow manual.
4. Sem assistente de IA na UI nova; mascote na Fase 5.
5. Windows é plataforma de primeira classe.

**Ainda dependem de você** (com a minha recomendação)
1. Nome da branch do núcleo: `feat/native-core`? → **Recomendo esse.**
2. A UI nova divide o `app.db` e as conexões com o Tauri? → **Recomendo sim**, com o
   estado da UI em arquivo próprio até a Fase 6.
3. GPUI por versão exata do crates.io em vez de commit git? → **Recomendo versão
   exata**; commit git só para um conserto não publicado (`rev = "<sha>"`).
4. Teto padrão de linhas numa query livre? → **Recomendo 100 mil**, com "carregar
   tudo"; abrir tabela continua paginado em 500.
5. Visual: portar os tokens do Ink (DESIGN.md) na Fase 0? → **Recomendo sim** (cores,
   tipografia, espaçamento; vidro e movimento depois).
6. Nome do binário e bundle id na beta? → **Recomendo `octapus-desktop` e
   `com.octapus-db.native`**, instalável lado a lado.
7. Onde testar Windows e Linux (máquina física, VM, qual distro/GPU)? → Sem
   recomendação: preciso saber.

---

## 9. DBFlux: o que adotar e o que é excesso

**Adotar (adaptado ao octapus)**
1. **UI nunca decide por id de driver** — já é a nossa regra (capabilities). Manter.
2. **Documentos com chave de dedup** (`DocumentKey`): abrir a mesma tabela foca a aba
   existente. Com ~6 tipos de documento, um `enum Document` fechado é mais simples que as
   closures apagadas do `PaneHandle` deles.
3. **Padrões GPUI:** trabalho pesado fora da UI e volta por `cx.spawn` + `update`;
   `Task` guardada em campo para cancelar no drop; campos `pending_*` consumidos no
   `render`; `_subscriptions: Vec<Subscription>`; formatar texto ao receber o dado, não a
   cada render.
4. **Keymap sobre os contexts nativos do GPUI** (predicados como `Editor && !Modal`),
   padrões + overrides do usuário, e **teste de cobertura de teclado**.
5. **Grade:** `uniform_list` no corpo, cabeçalho com rolagem horizontal sincronizada no
   mesmo frame, e uma trait de fonte de linhas (dados + buffer de edição) separada da
   pintura.
6. **Autocomplete** pelo `CompletionProvider` do gpui-component, com prefetch das
   colunas das tabelas citadas.
7. **Um ponto único para erro do usuário** (toast + contador na status bar).
8. **Tamanho de fonte por rem** sob o `Root`, com tamanhos separados para grade e editor;
   teste que reprova tamanho/cor crua (equivalente ao nosso `lint:tokens`).
9. **Sessão:** manifesto das abas persistido, só documentos de código; pendências não
   aplicadas perguntam antes de fechar.
10. **Confirmação de query perigosa** e **histórico de queries**.

**Excesso para o nosso escopo**
1. 60+ crates e 6 crates de UI → nós: 3.
2. Drivers externos por RPC, host de drivers, IPC de instância única.
3. Scripts Lua/Python/Bash, hooks de conexão, AWS/SSM/túneis/proxy.
4. Governança MCP, política, auditoria, aprovação (nosso MCP já é só-leitura no CLI).
5. Dashboards, gráficos, query builder visual, object storage, i18n em crate própria,
   design system em crate separada (`bolt`).
6. Drivers síncronos no executor do GPUI — nosso núcleo tokio fica como está.
7. Persistência em JSON — já temos o `app.db`.

---

## 10. Primeiros 5 passos

1. **Branch e worktree do núcleo** (§4.2): criar `feat/native-core` a partir da
   `feat/core-cli`, dar push, criar o worktree `../octapus_db-native-core`. ~15 min.
2. **Repositório privado** (§4.3): `git init`, `.gitignore`, `Cargo.toml` com o par atual
   `gpui-pre` + `gpui-component` (conferir no crates.io), `.cargo/config.toml` com o
   `[patch]`, `cargo check`. ~1 h.
3. **Janela + tema + núcleo:** `Root`, alternância claro/escuro, bridge tokio, lista dos
   servidores do seu `app.db`. ~1 dia.
4. **Spike da grade:** 1M linhas sintéticas; medir fps e memória no Mac. ~2 dias.
5. **Spike do editor + workflow manual:** editor SQL com highlight, teste de acentos
   pt-BR, `build.yml` manual rodado uma vez para Windows e Linux, e os dois spikes
   testados lá. ~2 dias.

---

## Perguntas em aberto

1. Qual era exatamente o problema do WebKitGTK (render, input, travamento, distro)? Isso
   define o que testar primeiro no Linux.
2. Há previsão de quando a `feat/core-cli` pode ir para a `main`? (Define quando a
   importação da Fase 6 pode acontecer.)
3. Você tem máquina ou VM Windows para testar a cada fase?
