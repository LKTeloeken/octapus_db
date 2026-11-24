# Design System — Abissal

Referência visual do **octapus_db**. Complementa o [FRONTEND.md](FRONTEND.md) (estrutura,
estado, componentes) com _como a interface deve parecer_: cor, tipografia, espaço,
elevação, movimento, vidro e componentes.

O Abissal substitui o Ink. Ele parte do que só o Octapus tem: um polvo que vive no fundo,
solta tinta, acende com bioluminescência e segura muita coisa ao mesmo tempo com
ventosas. A interface continua uma ferramenta de dados densa e legível; a personalidade
vem de poucos detalhes repetidos com rigor.

> **Fonte dos valores:** [`src/styles/globals.css`](src/styles/globals.css). Este
> documento fica com as regras de uso.
>
> **Especificação com exemplos:** [Abissal — Octapus](https://claude.ai/artifact/DXV1E7qksV6KSFy6GTfdUZ)
> (privado) — tokens, mockups dos componentes nos dois temas, auditoria e decisões. A UI
> nativa (GPUI) segue o mesmo sistema.

---

## 1. Princípios

1. **Os dados ficam nítidos, o resto fica fundo.** Superfícies escuras e tingidas recuam;
   valores, tipos e alterações pendentes ganham contraste. Nenhum efeito de marca passa
   por cima de uma célula.
2. **Tinta marca o que é seu.** O violeta `--tinta` é a mão do usuário: ação principal,
   seleção, foco, o que ele arrasta. Não é decoração.
3. **Biolume marca o que está vivo.** O ciano `--biolume` só aparece quando algo acontece
   agora: conexão aberta, consulta rodando, cursor do editor. Quando nada está vivo, a
   tela não tem ciano.
4. **Ventosas no lugar de ícones de estado.** O ponto redondo é a assinatura: não salvo,
   rodando, marcadores na scrollbar, separadores da barra de status, carregamento.
5. **Tentáculos ligam as coisas.** Linhas que conectam (guias da árvore, indicador da aba
   ativa, seleção) terminam arredondadas ou em curva. Nada termina em canto vivo.
6. **O polvo reage, não decora.** O mascote aparece onde o app espera ou explica algo
   (tela vazia, barra de status, sem resultado), nunca em cima de dados e nunca em loop
   quando nada acontece.

---

## 2. Cor

Tudo em **OKLCH**. Quatro temas: **Abissal** (escuro, padrão — `.dark`), **Nácar** (claro
— `:root, .light`), **Fossa** (preto OLED) e **Recife** (escuro quente). Fossa e Recife são
`.dark[data-theme='…']` e só trocam as superfícies e a rampa de texto. Todo token novo
nasce em par claro/escuro.

### 2.1 Neutros e camadas

Índigo profundo (matiz 278, croma 0,022–0,032) no escuro; pérola lilás (matiz 290) no
claro. `--hover`, `--active` e as linhas têm alfa, então funcionam sobre qualquer camada.

| Token            | Abissal                  | Nácar                        | Uso                                                  |
| ---------------- | ------------------------ | ---------------------------- | ---------------------------------------------------- |
| `--bg`           | `oklch(0.145 0.022 278)` | `oklch(0.945 0.012 290)`     | O abismo: janela, vão entre painéis, barra de status |
| `--surface-1`    | `oklch(0.178 0.026 278)` | `oklch(0.988 0.005 290)`     | Painéis: árvore, editor, grade                       |
| `--surface-2`    | `oklch(0.205 0.029 278)` | `oklch(0.965 0.01 290)`      | Cabeçalho da grade, toolbars, rodapé de diálogo      |
| `--surface-3`    | `oklch(0.24 0.032 278)`  | `oklch(1 0 0)`               | Menus, popovers, diálogos, tooltips                  |
| `--hover`        | `oklch(1 0 0 / 5%)`      | `oklch(0.2 0.05 282 / 4.5%)` | Hover sobre qualquer superfície; listras da grade    |
| `--active`       | `oklch(1 0 0 / 8.5%)`    | `oklch(0.2 0.05 282 / 8%)`   | Pressionado; item de menu sob o teclado              |
| `--line-subtle`  | `oklch(1 0 0 / 6%)`      | `… / 7%`                     | Linhas da grade, divisórias                          |
| `--line`         | `oklch(1 0 0 / 10%)`     | `… / 12%`                    | Contorno de painéis e sobreposições                  |
| `--line-strong`  | `oklch(1 0 0 / 18%)`     | `… / 22%`                    | Guias da árvore, divisória que precisa aparecer      |
| `--edge-control` | `oklch(0.53 0.035 282)`  | `oklch(0.62 0.035 282)`      | Borda de input, select, checkbox e switch (3:1)      |
| `--fg`           | `oklch(0.95 0.012 285)`  | `oklch(0.21 0.035 282)`      | Texto principal                                      |
| `--fg-muted`     | `oklch(0.79 0.028 282)`  | `oklch(0.44 0.035 282)`      | Texto secundário, ícones                             |
| `--fg-subtle`    | `oklch(0.7 0.032 282)`   | `oklch(0.52 0.035 282)`      | Metadados, `NULL`, números de linha, tipo da coluna  |
| `--fg-disabled`  | `oklch(0.46 0.025 280)`  | `oklch(0.72 0.02 285)`       | Só controles desabilitados (isentos de contraste)    |

**Camadas**, de baixo para cima: `--bg` → `--surface-1` → `--surface-2` → `--surface-3`.
Uma camada nunca pula outra.

### 2.2 Tinta e Biolume

| Token            | Abissal                     | Nácar                        | Uso                                                        |
| ---------------- | --------------------------- | ---------------------------- | ---------------------------------------------------------- |
| `--tinta`        | `oklch(0.56 0.2 295)`       | `oklch(0.5 0.22 295)`        | Preenchimento: botão primário, checkbox, switch, tentáculo |
| `--tinta-hover`  | `oklch(0.52 0.2 295)`       | `oklch(0.45 0.22 295)`       | Hover do preenchimento                                     |
| `--tinta-text`   | `oklch(0.8 0.13 295)`       | `oklch(0.48 0.22 295)`       | Acento como texto/ícone: links, casamento na paleta        |
| `--tinta-soft`   | `oklch(0.62 0.2 295 / 20%)` | `oklch(0.55 0.22 295 / 12%)` | Seleção: linhas, células, nó aberto na árvore              |
| `--on-tinta`     | branco                      | branco                       | Texto sobre `--tinta` (o abismo nos acentos Biolume, Âmbar) |
| `--ring`         | `= --tinta-text`            | `= --tinta-text`             | Anel de foco, sólido (≥ 3:1)                               |
| `--biolume`      | `oklch(0.86 0.13 182)`      | `oklch(0.5 0.1 190)`         | Sinal de vida: conexão viva, consulta rodando, cursor      |
| `--biolume-soft` | `… / 14%`                   | `… / 14%`                    | Halo do sinal de vida (faixa no editor, chip da conexão)   |

**Tinta tem quatro papéis — e só estes:** ação primária (uma por área), foco de teclado,
seleção de dados, controle ligado. Também é o tentáculo da aba ativa, a pílula do item
ativo (árvore, paleta) e o polegar da scrollbar arrastada. **Não use** em títulos, ícones
de pasta, fundos grandes ou gradientes. Orçamento: até ~5% da tela.

**Biolume só aparece quando algo está vivo.** Nunca em botão, nunca parado em algo que
não está acontecendo.

### 2.3 Semânticas

Sólido (ícone, ponto, texto) e `-soft` (fundo). No claro os sólidos escurecem para manter
4,5:1 também sobre o próprio `-soft`.

| Token            | Abissal                | Nácar                 | Uso                                         |
| ---------------- | ---------------------- | --------------------- | ------------------------------------------- |
| `--success`      | `oklch(0.79 0.15 152)` | `oklch(0.5 0.13 152)` | Concluído, linha nova (`+`)                 |
| `--warning`      | `oklch(0.85 0.14 80)`  | `oklch(0.51 0.12 68)` | Célula editada (`•`), aviso, chave primária |
| `--danger`       | `oklch(0.72 0.18 22)`  | `oklch(0.53 0.2 25)`  | Erro, linha removida (`−`), produção        |
| `--danger-solid` | `oklch(0.55 0.19 25)`  | `oklch(0.53 0.2 25)`  | Preenchimento destrutivo (branco em cima)   |
| `--info`         | `oklch(0.79 0.11 240)` | `oklch(0.5 0.14 245)` | Mensagens do servidor, dicas                |

### 2.4 Alterações pendentes na grade

Codificação dupla — cor **e** forma —, legível para daltônicos.

| Estado            | Fundo                         | Gutter                   | Texto                   |
| ----------------- | ----------------------------- | ------------------------ | ----------------------- |
| Célula editada    | `--warning-soft` só na célula | `•` em `--warning`       | cor do tipo             |
| Linha nova        | `--success-soft` na linha     | `+` em `--success`       | cor do tipo             |
| Linha removida    | `--danger-soft` na linha      | `−` em `--danger`        | `--fg-subtle` + tachado |
| Linha selecionada | `--tinta-soft`                | número em `--tinta-text` | —                       |

A seleção nunca apaga o estado pendente. Depois do `ok` a tinta some em 600 ms. A busca
(Cmd/Ctrl+F) destaca o trecho com `--match-soft` e o resultado atual com `--match`.

### 2.5 Identidade dos bancos

`--db-postgres`, `--db-mongo`, `--db-redis`, `--db-sqlite`, `--db-mysql`: só no ícone do
servidor, da aba, da barra de status e em pontos de 6 px. Nunca fundo, nunca texto corrido.

### 2.6 Tipos de dado na grade

`--data-number` (à direita, `tabular-nums`), `--data-bool`, `--data-date`, `--data-json`;
texto em `--fg`; `NULL` em `--fg-subtle` itálico; uuid/binário em `--fg-muted`.

### 2.7 Sintaxe (CodeMirror)

`--syn-keyword` (= tinta-text), `--syn-function` (tom biolume), `--syn-string`,
`--syn-number`, `--syn-type`; comentário `--fg-subtle` itálico; operadores `--fg-muted`.

### 2.8 Vidro e scrim

`--glass` (surface-3 a ~84%), `--glass-border`, `--glass-highlight`, `--scrim` (véu sob
diálogos e paleta). Ver §7.

### 2.9 Orçamento de cor

Numa tela de tabela: superfícies ~70% · texto e ícones ~22% · dados e bancos ~4% · tinta
~3% · semânticas e biolume ~1%.

### 2.10 Barra de título

Desenhada pelo app: no macOS, o semáforo em `--traffic-rest` que ganha as cores do
sistema no hover; no Linux, botões `ghost` no fim da linha de abas; no Windows, a barra do
sistema. O cabeçalho da sidebar, a linha de abas e a barra de status arrastam a janela.

---

## 3. Tipografia

| Família                                        | Uso                                                                  | Pacote                                     |
| ---------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------------ |
| **Bricolage Grotesque**                        | A voz da marca: títulos de diálogo, estados vazios, rótulos de seção | `@fontsource-variable/bricolage-grotesque` |
| **Geist** (ou Inter, do sistema)               | Toda a interface                                                     | `@fontsource-variable/geist`               |
| **Geist Mono** (ou JetBrains Mono, do sistema) | Dados, SQL, tipos, atalhos                                           | `@fontsource-variable/geist-mono`          |

Empacotadas no [`main.tsx`](src/main.tsx) — o app roda offline. A fonte da interface e a
mono vêm de `--app-font-ui` / `--app-font-mono`, trocadas nas preferências.

| Token          | Tam. / linha | Peso | Uso                                               |
| -------------- | ------------ | ---- | ------------------------------------------------- |
| `text-display` | 28 / 32      | 700  | Tela vazia grande (`font-display`), um por tela   |
| `text-title`   | 17 / 24      | 600  | Título de diálogo e de painel (`font-display`)    |
| `text-section` | 12 / 16      | 600  | Rótulo de grupo em caixa normal (`font-display`)  |
| `text-heading` | 14 / 20      | 600  | Seções dentro de painel                           |
| `text-body`    | 13 / 20      | 400  | Padrão: árvore, menus, abas                       |
| `text-small`   | 12 / 16      | 400  | Metadados, barra de status, ajuda                 |
| `text-cell`    | densidade    | 400  | Células da grade (12 / 12,5 / 13 px), `font-mono` |
| `text-micro`   | 11 / 14      | 500  | Só onde não cabe outro (contadores)               |

Regras: hierarquia por cor primeiro, depois peso, por último tamanho. `tabular-nums` em
todo número alinhado. Mono só para valores. Itálico só em `NULL` e comentário. Sem
MAIÚSCULAS em rótulos (exceção: a palavra PRODUÇÃO no chip da conexão). Bricolage nunca
abaixo de 12 px e nunca em dados.

---

## 4. Espaço, densidade e raio

**Espaço:** base de 4 px (`--spacing: 0.25rem`); o vão entre painéis é 4–8 px.

**Densidade** (`[data-density]` no `<html>`, escolhida nas preferências):

| Densidade   | Linha da grade e da árvore (`--row-h`, `--tree-row-h`) | Fonte das células (`--cell-text`) |
| ----------- | ------------------------------------------------------ | --------------------------------- |
| Compacta    | 24 px                                                  | 12 px                             |
| Normal      | 28 px (padrão)                                         | 12,5 px                           |
| Confortável | 32 px                                                  | 13 px                             |

Listas virtualizadas leem a altura de `useDensityRows()` (`stores/ui-store.ts`), que
espelha os mesmos valores.

**Fixos:** cabeçalho da grade 40 (nome + tipo), linha de abas 38 com abas de 32, barra de
status 32, toolbar do editor 44, controles `xs` 24 · `sm` 28 (padrão) · `default` 32 · `lg` 36.

**Raio:** `rounded-xs` 3 (cursor de célula, kbd) · `rounded-sm` 7 (botões, inputs, linhas)
· `rounded-md` 9 (abas) · `rounded-lg` 12 (painéis, menus, toasts) · `rounded-xl` 18
(diálogos, paleta) · `rounded-full` (ventosas, chips, polegar). Todo indicador linear
(seleção, aba ativa, instrução rodando, progresso) é uma **pílula**.

---

## 5. Elevação, bordas e foco

| Nível | Utilitário       | Uso                                              |
| ----- | ---------------- | ------------------------------------------------ |
| 0     | —                | Painéis                                          |
| 1     | `shadow-raised`  | Aba ativa, botão secundário, polegar do segmento |
| 2     | `shadow-overlay` | Menus, popovers, select, tooltip                 |
| 3     | `shadow-modal`   | Diálogo, paleta, toast                           |
| —     | `shadow-live`    | Halo biolume (`--glow-live`)                     |

Bordas de 1 px com alfa (`--line-subtle`, `--line`, `--line-strong`); controles de
formulário usam `--edge-control` (3:1). **Foco:** um anel por contexto, sólido em
`--ring`, só com `:focus-visible` — botões com 2 px de respiro, inputs com borda
`--tinta-text` + halo `--tinta-soft`, células e linhas com anel inset de 1,5 px.

---

## 6. Movimento

Durações: 0 (seleção, teclado, scroll, troca de aba) · 80 ms (press, escala .97) · 120 ms
(hover, saídas) · 180 ms (menus, árvore, tentáculo da aba) · 240 ms (diálogo, paleta,
toast) · 600 ms (tinta de alteração salva). Curvas `ease-standard`, `ease-out`, `ease-in`,
`ease-in-out`, `ease-spring`. Só `transform`, `opacity` e cor.

**Momentos de assinatura:**

| Momento           | O que faz                                                            | Onde                                     |
| ----------------- | -------------------------------------------------------------------- | ---------------------------------------- |
| Jato de tinta     | Anel `--tinta-text` sai do botão Executar e some (`animate-ink-jet`) | `query-editor-toolbar`                   |
| Ventosas          | Cinco pontos em arco acendem em sequência (`animate-suck`)           | `ui/spinner.tsx` — o carregando em linha |
| Pulso biolume     | Halo que respira enquanto roda (`animate-live`)                      | Ponto da aba, chip da conexão            |
| Faixa biolume     | Linhas executadas com `--biolume-soft` + pílula de 3 px              | `query-editor/running-band.ts`           |
| Tentáculo da aba  | Pílula `--tinta` de 2 px no topo da aba ativa (fade 180 ms)          | `ui/tabs.tsx`                            |
| Tinta assentando  | A cor da alteração pendente some em 600 ms depois do `ok`            | Grade                                    |
| Scrollbar respira | Aparece no hover da área, engorda de 4 para 8 px, tinta ao arrastar  | `globals.css`                            |

**Nunca animar:** navegação por setas, scroll virtualizado, digitação, autocompletar,
chegada de resultados. Nada em loop fora de carregamento e sinal de vida. **Movimento
reduzido:** deslocamento e escala viram fade de 120 ms; o jato some; o pulso fica aceso
parado; as ventosas só piscam.

**O polvo.** Parado (`OctapusIdle`) na tela sem abas e na barra de status quando ocioso;
trabalhando (`OctapusThinking`) na barra de status enquanto algo roda; coçando a cabeça
(`OctapusAnimated scene="scratch"`) na paleta sem resultado; as cenas do assistente
continuam. Sempre em `currentColor` (`text-tinta-text`). Preferência: Sempre, Quando
ocupado (padrão: some da barra de status quando ocioso) ou Nunca.

---

## 7. Vidro

Só em camadas temporárias sobre conteúdo: paleta de comandos, toasts, barra de alterações
pendentes. Menus, tooltips, diálogos de formulário, painéis e barras são sólidos. Nunca
vidro sobre vidro. Utilitário `glass`; com `prefers-reduced-transparency`, cai para
`--surface-3`.

---

## 8. Componentes

Primitivos em `src/components/ui/` (Radix + `cva`). Componente de tela não recria botão,
badge ou atalho.

- **Scrollbar** — global no `globals.css`: sobreposta, some em repouso, 4 px no hover da
  área, 8 px no hover da faixa (12 px de área de clique), `--tinta` ao arrastar. A grade
  pinta **ventosas no trilho** (`results-table-scroll-markers`): resultados da busca em
  `--match` e linhas pendentes em `--warning`/`--success`/`--danger`.
- **Abas** (`ui/tabs.tsx`, `features/query-tabs`) — documento: a ativa em `--surface-1` +
  `shadow-raised` + tentáculo `--tinta` no topo; ícone na cor do banco. O controle à
  direita é × na ativa e no hover; nas outras, uma ventosa diz o estado: `--biolume`
  pulsando (rodando) ou `--fg-muted` (não salva). Arrastar reordena; barra `--tinta`
  marca onde cai; quando transbordam, o botão ▾ lista todas.
- **Árvore** (`features/connection-tree`) — linha da densidade, chevron que gira, ícones
  `--fg-subtle` (servidor na cor do banco). **Guias de tentáculo** (`tree-guides.ts`):
  linha de 1 px `--line-strong` por nível; o último filho termina numa curva. Nó aberto
  na aba ativa: `--tinta-soft` + pílula `--tinta` de 3 px. Servidor que já respondeu:
  ventosa `--biolume` depois do nome. Foco de teclado: anel inset.
- **Grade** (`components/results-table`) — cabeçalho de 40 px com nome, tipo e PK;
  ordenação ativa em `--tinta-text`; grip de redimensionar em pílula `--tinta-text` no
  hover. Linhas com hover `--hover` e listras opcionais. Estados pendentes como §2.4.
- **Editor** — cursor `--biolume`; faixa biolume na instrução rodando; autocompletar com
  chip do tipo, letras casadas em `--tinta-text`; rótulos de seção em Bricolage.
- **Barra de status** (`features/status-bar`) — 32 px sobre o abismo: polvo, chip da
  conexão da aba ativa (pulsa em biolume enquanto roda; grupo com "prod" no nome ⇒
  `--danger-soft` + PRODUÇÃO), "Executando · 1,2 s" ou "Pronto" e o atalho da paleta.
  Linhas e tempo ficam no rodapé da grade, sem repetir aqui.
- **Paleta de comandos** — vidro, `rounded-xl`, rótulos de grupo em `text-section`, item
  ativo `--active` + pílula `--tinta`, letras casadas em `--tinta-text`, polvo coçando a
  cabeça quando não há resultado.
- **Menus, popovers, select** — sólidos, `--surface-3`, `shadow-overlay`, `rounded-lg`;
  item ativo `--active` (nunca tinta); destrutivo em `--danger`, por último.
- **Diálogos** — sólidos, `--surface-3`, `rounded-xl`, título em `font-display
  text-title`, rodapé em `--surface-2`. Destrutivo: `--danger-solid`.
- **Toasts** (`components/Toaster.tsx`) — vidro, selo redondo de 30 px no tom
  (`-soft`), fechar; no máximo três; sucesso some em 4 s, erro fica até fechar.
- **Botões** — primário `--tinta` + `--on-tinta`; secundário `--control`; ghost; perigo
  `--danger-soft` → `--danger-solid`; press `scale(.97)`.
- **Campos** — fundo `--field` (o abismo), borda `--edge-control`, hover `--fg-subtle`,
  foco `--tinta-text` + halo `--tinta-soft`; erro em `--danger`.
- **Estados vazios** (`ui/content-state.tsx`) — selo redondo `--tinta-soft` (ou
  `--danger-soft`), título em Bricolage; `mascot` troca o selo pelo polvo, título
  `text-display` e alinha à esquerda no terço de cima (tela sem abas).
- **Carregando** — ventosas em linha (`Spinner`), barra indeterminada de 2 px no topo do
  painel e esqueleto parado só depois de 400 ms.
- **Iconografia** — só **Hugeicons**, traço 1,5, 16 px (14 em linhas densas). Cor padrão
  `--fg-muted`; cor de banco só em servidor/aba/chip.

---

## 9. No código

Tokens em [`src/styles/globals.css`](src/styles/globals.css): `:root, .light` (Nácar),
`.dark` (Abissal), `.dark[data-theme]` (Fossa, Recife), `[data-accent]` (acentos),
`[data-density]`, `[data-ui-font]`, `[data-mono-font]` — todos aplicados no `<html>` pelo
`app.tsx` a partir do `ui-store`. Expostos ao Tailwind v4 no `@theme inline`.

| Categoria   | Utilitários                                                                                          |
| ----------- | ---------------------------------------------------------------------------------------------------- |
| Superfícies | `bg-bg`, `bg-surface-1/2/3`, `bg-hover`, `bg-active`, `bg-field`, `bg-control`                       |
| Linhas      | `border-line-subtle`, `border-line`, `border-line-strong`, `border-edge-control`                     |
| Texto       | `text-fg`, `text-fg-muted`, `text-fg-subtle`, `text-fg-disabled`                                     |
| Tinta       | `bg-tinta`, `hover:bg-tinta-hover`, `text-tinta-text`, `bg-tinta-soft`, `text-on-tinta`, `ring-ring` |
| Biolume     | `bg-biolume`, `bg-biolume-soft`, `shadow-live`, `animate-live`                                       |
| Semânticas  | `text-success`, `bg-success-soft`, … (`warning`, `danger`, `info`), `bg-danger-solid`                |
| Tipo        | `font-display`, `text-section`, `text-cell`, `text-small` … `text-display`                           |
| Movimento   | `animate-suck`, `animate-live`, `animate-ink-jet`, `ease-*`, `duration-120/180/240`                  |

**Token novo:** (1) valor em `:root, .light` **e** `.dark`; (2) `--color-*` no `@theme
inline`; (3) nome novo de tamanho de texto, sombra, fonte ou animação também no
`extendTailwindMerge` de [`lib/utils.ts`](src/lib/utils.ts) — sem isso o `cn()` descarta a
classe ao lado de uma cor; (4) par novo de contraste no `contrast.test.ts`; (5) registre
aqui. Cor usada fora do CSS (CodeMirror, toasts) é `var(--…)`, nunca hex em TSX.

---

## 10. Personalização

Preferências › Aparência (`features/settings/sections/appearance-section.tsx`), guardadas
no `ui-store` (`octapus-ui`, versão 1 — a 0 do Ink migra `dark` → Abissal, `light` →
Nácar):

- **Tema:** Abissal, Nácar, Fossa, Recife ou seguir o sistema (Abissal/Nácar).
- **Acento:** Tinta (padrão), Coral, Biolume, Alga, Âmbar, Azul. Cada um troca `--tinta*`,
  `--ring` e `--syn-keyword`; Biolume e Âmbar no escuro usam o abismo como `--on-tinta`.
  Com acento Biolume, o sinal de vida no claro passa para `--success`.
- **Densidade:** Compacta, Normal, Confortável.
- **Fontes:** interface (Geist, Inter, do sistema) e dados (Geist Mono, JetBrains Mono, do
  sistema).
- **Listras na grade** e **Polvo** (Sempre, Quando ocupado, Nunca).

O `contrast.test.ts` confere os quatro temas e os seis acentos.

---

## 11. Checklist de PR

1. Nenhuma cor crua em TSX — só tokens. O `pnpm lint:tokens` verifica.
2. Tinta só nos papéis do §2.2; biolume só no que está vivo.
3. Estado novo nasce em par claro/escuro e passa 4,5:1 (3:1 para ícones e bordas de
   controle) — o `src/styles/contrast.test.ts` calcula a partir do `globals.css`.
4. Animar só `transform`, `opacity` e cor, com as durações e curvas do tema; nada de
   `transition-all`.
5. Vidro só via `glass`, e só em camadas flutuantes (§7).
6. Texto mínimo de 11 px; `tabular-nums` em números; valores em mono; rótulos sem
   MAIÚSCULAS.
7. Um conjunto de ícones (Hugeicons), 16 px, traço 1,5.
8. Linha de lista nova segue a densidade (`h-(--row-h)` / `useDensityRows()`).
9. Componente de tela usa `Button`/`Badge`/`Kbd`/`Spinner`/`ContentState` de
   `components/ui` — não recria.
