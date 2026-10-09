# Modo mock — backend simulado

Permite rodar **só o front** (`pnpm dev:mock`, porta 1430) sem compilar o Rust.
Nada fora desta pasta sabe que o modo existe: o mock é instalado no nível do IPC do
Tauri, então `src/api/`, `src/queries/` e as features rodam exatamente como em produção.

```
pnpm dev:mock    # front sozinho, backend simulado, http://localhost:1430
pnpm tauri dev   # app completo com o Rust real, http://localhost:1420
```

As duas portas são origins diferentes, então `localStorage` e o cache do React Query em
IndexedDB ficam isolados — mexer no mock não contamina o app real.

## Como funciona

`src/main.tsx` só carrega esta pasta quando `import.meta.env.VITE_MOCK === 'true'`
(definido em `.env.mock`). Em `pnpm build` a variável não existe, o `if` vira código morto
e o `import()` some do bundle.

`installMocks()` chama o `mockIPC` oficial do `@tauri-apps/api/mocks`, que preenche
`window.__TAURI_INTERNALS__.invoke` e `.transformCallback`. Com isso funcionam sem o Rust:

- o `invoke` de [`src/api/client.ts`](../api/client.ts);
- o `Channel<QueryMessage>` de [`src/api/query.ts`](../api/query.ts);
- o `check()` do `@tauri-apps/plugin-updater`.

```
invoke(cmd, args)
  └─ router.ts     latência + injeção de erro (painel)
      └─ handlers.ts  um handler por RustCommand
          └─ engine.ts   sort / where / paginação / mutações
              └─ data/    dataset em memória, gerado com seed fixa
```

**Rejeição é sempre `string`**, como o `.map_err(|e| e.to_string())` dos comandos Rust —
é o que o `call()` espera para montar o `ApiError`.

**Os argumentos chegam sem serialização.** Como o `invoke` interno é substituído,
`args.messages` é a instância de `Channel`, não a string `__CHANNEL__:id`; para emitir uma
mensagem basta `channel.onmessage(payload)`.

## Catálogo e eventos

Os comandos `catalog_*` vão para [`catalog.ts`](catalog.ts), um `CatalogService` em
pequeno: sincroniza em camadas emitindo `catalog-event` (`syncing` → `schemas` →
`relations` → `ready`), carrega na hora o schema pedido antes da sincronização chegar,
agrupa a busca por nome e resolve tabela sem schema. A forma de cada database vem do
dataset; Mongo e SQLite, que não têm schema, ficam todos no schema sem nome
(`FLAT_SCHEMA`), como no backend. Só o Redis responde "sem catálogo".

O mock também agrupa por formato (`catalog_shapes` e o caminho `{ kind: 'shape' }`), com
o mesmo critério do backend (Jaccard ≥ 0,5 com o molde), marca o drift de cada schema e
monta o `catalog_diagnostics`. O escopo salvo no servidor (`scopeDatabases`,
`scopeSchemas`) vale como no backend ([`scope.ts`](scope.ts)): filtra a lista de
databases e os schemas do catálogo, e editar o servidor zera os catálogos dele.

Para desenhar em escala, o Postgres do mock traz o database **`saas`**
([`data/tenants.ts`](data/tenants.ts)): 5.000 schemas `tenant_NNNNN` com as mesmas 150
tabelas, como a fixture de `perf/catalog/` (múltiplos de 100 sem as 3 últimas). Nada é
gerado de antemão; uma tabela só vira `MockTable` quando alguém a abre. O Mongo traz
ainda 2.000 databases `tenant_NNNN` (um database por cliente), para o filtro e a janela
do nível do servidor.

Eventos globais (`listen` do `@tauri-apps/api/event`) passam por
[`events.ts`](events.ts): `plugin:event|listen` registra o callback e `mockEmit(evento,
payload)` o dispara. O reset do painel também zera o catálogo.

## Painel de dev

Badge `MOCK` no canto inferior direito. Controla latência, injeção de erro
(`nunca` / `próximo` / `sempre`, com mensagem editável), respostas vazias (para desenhar
empty states), update fake e reset do dataset. A config fica em `localStorage`
(`octapus-mock`); só o modo de falha não sobrevive ao reload, de propósito.

## Receitas

### Adicionar uma tabela ao dataset

Em `data/postgres.ts` (ou `mongo.ts` / `redis.ts`), declare o `MockTable` e inclua-o no
`buildXDatabases()`. As linhas são materializadas no primeiro acesso a partir do
`gen` de cada coluna — veja os geradores prontos em `data/rows.ts` (`serial`, `fk`,
`textArray`, `json`, `timestamp`, `email`, …).

```ts
const assinaturas: MockTable = {
  name: 'assinaturas',
  schema: 'public',
  tableType: 'table',
  rowEstimate: 2_000,
  columns: [
    pk(),
    fkColumn('usuario_id', USUARIOS, { isNullable: false }),
    column('plano', 'text', OID.text, gen.pick(['free', 'pro'])),
  ],
  indexes: [],
};
```

Tabela sem coluna `isPrimaryKey`, ou com `tableType` diferente de `'table'`, volta com
`editableInfo: null` — é assim que se testa o grid em modo leitura.

### Adicionar um comando novo

Quando um `#[tauri::command]` novo entra em `src-tauri/src/lib.rs` e vira um valor do enum
`RustCommand` em [`src/api/commands.ts`](../api/commands.ts), adicione o handler
correspondente em `handlers.ts`. Sem handler, o mock rejeita com
`Unsupported command: "…"` — o erro diz exatamente o que falta.

### Simular um erro específico

Handler que rejeita direto com a string do backend:

```ts
throw 'Connection error: connection refused';
```

Para um erro pontual sem mexer em código, use o painel (`falhar: próximo`).
