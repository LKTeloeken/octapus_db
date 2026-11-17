# Executor de rodadas do experimento (TCC)

Ferramenta **privada do autor**: roda a bateria de perguntas no assistente de
verdade, grava cada teste e mostra tudo numa tela local. Fica atrás da feature
`experiment` e entra como um *example* do Cargo, então não vai para o app nem
para o instalador.

Todos os comandos rodam de `src-tauri/`:

```bash
cargo run --example experiment --features experiment -- --help
```

## Comandos

| comando | o que faz |
|---|---|
| `check --questions ARQ [--env ARQ]` | Validação prévia. Sem `--env`: formato, idioma e gabarito só-leitura. Com `--env`: executa cada gabarito duas vezes. Não chama a API. |
| `run --questions ARQ --env ARQ --dry-run` | Só informa quantos testes rodariam. |
| `run --questions ARQ --env ARQ [--out PASTA] [--only id,id]` | Rodada real. Pede para digitar o número de testes num terminal; não há opção que pule isso. |
| `resume PASTA [--redo-failed]` | Continua de onde parou, sem repetir. `--redo-failed` refaz os turnos que terminaram em `failed`. |
| `report PASTA` | Regrava `results.csv` e `summary.csv`. |
| `serve [--runs PASTA] [--port N]` | Abre a tela em `http://127.0.0.1:7878`. Só lê a pasta. |
| `demo [--delay-ms N] [--concurrency N]` | Rodada simulada num SQLite descartável, sem rede nem cota. |

`Ctrl+C` interrompe: os turnos em andamento são cancelados (não valem como
resultado) e o que já foi gravado fica. A pasta padrão das rodadas é
`experiment-runs/`, que se ignora sozinha no git.

## Entradas

**Perguntas** (`questions.rs` tem o formato completo):

```toml
version = "2026-11"

[defaults.compare]
decimals = 2

[[question]]
id = "q001"
difficulty = "easy"
pt = "Quantos clientes existem no cadastro?"
en = "How many customers are there in the records?"
[question.gold.postgres]
query = "SELECT count(*) FROM customers"
[question.gold.mongodb]
query = '''{ "collection": "customers", "operation": "countDocuments" }'''

[[question]]
id = "q050"
difficulty = "hard"
pt = "Qual é a cor preferida de cada cliente?"
en = "What is each customer's favorite color?"
unanswerable = true   # o acerto é o turno pedir esclarecimento
```

O gabarito é escrito na linguagem que o assistente gera para o banco (SQL, a
operação do Mongo, o plano do Redis) e passa pelo guard do assistente: escrita
é recusada. O idioma de cada texto é conferido pelo detector do app.

**Ambiente** (`config.rs`):

```toml
[api]
invite_env = "OCTAPUS_E2E_INVITE"   # o NOME da variável, nunca o valor
models = ["modelo-a", "modelo-b"]   # ids dos modelos na API

[run]
repetitions = 3
seed = 42
concurrency = 1                     # padrão: sequencial
ask_user = "clarify"                # ou "reply" + ask_user_reply = { pt = "...", en = "..." }
mongo_sample_values = false
row_limit = 200                     # a página do app

[database.postgres]
host = "localhost"
username = "postgres"
password_env = "OCTAPUS_EXPERIMENT_PG_PASSWORD"
database = "loja"

[database.mongodb]
uri_env = "OCTAPUS_EXPERIMENT_MONGO_URI"
database = "loja"

[database.redis]
host = "localhost"
port = 6380
database = "2"
```

Senha, token, convite e URI nunca ficam no arquivo. A API é a de
`OCTAPUS_ASSISTANT_URL`, como no app (ou `url` em `[api]`). O participante
precisa estar no modo de escolha livre de modelo, para a rodada fixar o modelo
de cada teste.

## Como um teste roda

Cada unidade (pergunta, idioma, modelo, paradigma, repetição) abre uma conversa
nova, com o modelo do teste, e chama o `turn::run_turn` do app com a execução
automática ligada. As portas do turno são embrulhadas só para anotar; nada do
assistente foi alterado. A ordem é em blocos (uma pergunta com todas as suas
condições embaralhadas), reproduzível pela semente.

## Veredito

Compara o **resultado** obtido com o do gabarito, nunca o texto da consulta.
Regras, com padrão no arquivo e ajuste por pergunta ou por gabarito
(`compare = { ... }`):

| regra | valores | padrão |
|---|---|---|
| `order` | `ignore` (multiconjunto), `strict` (mesma sequência) | `ignore` |
| `columns` | `any` (pareia por conteúdo, ignora nomes), `position`, `name` | `any` |
| `extra_columns` | `forbid`, `allow` (faltar coluna sempre reprova) | `forbid` |
| `decimals` | casas no arredondamento de números | `4` |
| `null_equals_empty` | nulo igual a texto vazio | `false` |
| `truncated` | `fail`, `subset` (obtido cortado pela página) | `fail` |

- Pergunta sem resposta possível: acerto = o turno terminar em esclarecimento.
- **Acerto na primeira tentativa** = acertou sem nenhuma correção. **Acerto
  final** = acertou ao fim do turno. Os dois ficam gravados.
- **Correspondência exata**: só PostgreSQL. Igualdade da sequência de tokens,
  sem contar espaços, comentários, `;` final e caixa de palavras-chave e de
  identificadores sem aspas. Não entra no veredito.

## O que a rodada grava

| arquivo | conteúdo |
|---|---|
| `manifest.json` | versão do app, commit, versão e hash de cada prompt, modelos, semente, política de pergunta ao usuário, valores de exemplo do MongoDB, o conjunto de perguntas e o ambiente como foram lidos |
| `gold.json` | o resultado esperado de cada gabarito |
| `units.jsonl` | um teste concluído por linha: estado final, tempo total do turno, cada tentativa (consulta, etapa e classe de falha, veredito, linhas, tempos, tokens, id na API) |
| `discarded.jsonl` | testes que não abriram ou foram interrompidos |
| `live.json` | o andamento agora |
| `results.csv` | formato longo, um teste por linha; pareie por `question_id` + `repetition` |
| `summary.csv` | por paradigma, modelo e idioma: acurácia na 1ª tentativa e final, consultas inválidas, tempos, correspondência exata |

**Consultas inválidas** = das tentativas que geraram consulta, as que não
rodaram até o fim (guard, preflight ou execução). Saída fora do contrato é
contada à parte (`invalid_outputs`).

## Regras que não mudam

- **Só leitura**: tudo que toca um banco passa pelo guard e pelo executor
  só-leitura do assistente, inclusive o gabarito.
- **Log sem conteúdo**: pelo log do projeto (`--verbose`) só ids, contagens e
  códigos. Pergunta e consulta ficam só nos arquivos da rodada.
- **A tela só observa**: `GET` em `127.0.0.1`, sem rota que escreva.

## Testes

```bash
cargo test --features experiment --lib experiment
```

Sem rede: comparador, carregador de perguntas, plano, retomada, política de
pergunta ao usuário e o fluxo completo com modelo e registro simulados. Os que
dependem do ambiente real são `#[ignore]` (ver o fim de `tests.rs`):
`e2e_gold` só lê os bancos; `e2e_real` roda três turnos na API e gasta cota.

## Depois do merge da `feat/core-cli`

`setup.rs` é uma cópia dos auxiliares do `assistant_turn_ask`, que na `main`
são privados em `commands/assistant.rs`. A `feat/core-cli` já os moveu para
`assistant/setup.rs`: depois do merge, apague `setup.rs` e use o módulo de lá.
O lugar natural da ferramenta passa a ser um crate próprio no workspace
(`crates/octapus-experiment`), sem o Tauri.
