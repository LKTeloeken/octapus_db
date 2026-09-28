# Design System — Ink

Referência visual do **octapus_db**. Complementa o [FRONTEND.md](FRONTEND.md) (estrutura,
estado, componentes) com *como a interface deve parecer*: cor, tipografia, espaço,
elevação, movimento, vidro e componentes.

> **Status:** proposta aprovada, migração ainda não iniciada. O
> [`globals.css`](src/styles/globals.css) ainda tem os tokens antigos (tema shadcn +
> violeta saturado). Até a Fase 1 do [roteiro](#11-roteiro) aterrissar, os valores deste
> documento são a fonte da verdade; depois, o `globals.css` passa a ser a fonte dos valores
> e este documento fica com as regras de uso.
>
> **Canvas:** [Ink — Octapus Design System](https://claude.ai/artifact/V3efoQX3WcX2peLjivNm4k)
> (privado) — as mesmas regras em pranchas, com as telas redesenhadas e as demos de movimento.

---

## 1. Princípios

1. **Os dados são o protagonista.** A interface recua para que valores, tipos e alterações
   pendentes ganhem contraste. Cromo neutro; cor no conteúdo, por significado.
2. **Cor só com significado.** Iris (o violeta da marca) marca ação principal, foco e
   seleção. O resto vem de neutros, semânticas, bancos e tipos de dado. Orçamento: ≤ 5%
   da tela.
3. **Profundidade por camadas.** Quatro superfícies com passos de luminosidade e bordas
   finas no lugar de sombras iguais.
4. **Movimento que explica.** 120–240 ms, mostrando de onde algo veio e para onde foi.
   Teclado, grade e seleção nunca esperam animação.
5. **Vidro só no que flutua.** Blur apenas em camadas temporárias sobre conteúdo. Nunca em
   painéis fixos, nunca vidro sobre vidro.

---

## 2. Cor

Tudo em **OKLCH**. O tema escuro é o padrão (`ui-store`), mas todo token existe nos dois
temas — estado novo nasce em par claro/escuro.

### 2.1 Neutros e camadas

Slate frio (matiz 265, croma ≤ 0,014). `--hover`, `--active` e as linhas são branco/preto
com alpha, então funcionam sobre qualquer camada.

| Token | Escuro | Claro | Uso |
|---|---|---|---|
| `--bg` | `oklch(0.165 0.006 265)` | `oklch(0.955 0.004 265)` | Fundo da janela |
| `--surface-1` | `oklch(0.2 0.007 265)` | `oklch(0.995 0.001 265)` | Painéis: sidebar, área principal |
| `--surface-2` | `oklch(0.228 0.008 265)` | `oklch(0.975 0.003 265)` | Header da grade, toolbars, rodapé de diálogo |
| `--surface-3` | `oklch(0.26 0.009 265)` | `oklch(1 0 0)` | Menus, popovers, diálogos, tooltips |
| `--hover` | `oklch(1 0 0 / 4.5%)` | `oklch(0 0 0 / 4%)` | Hover sobre qualquer superfície |
| `--active` | `oklch(1 0 0 / 7.5%)` | `oklch(0 0 0 / 7%)` | Pressionado; aba/linha aberta; item ativo de menu |
| `--line-subtle` | `oklch(1 0 0 / 5.5%)` | `oklch(0 0 0 / 6%)` | Linhas da grade, divisórias |
| `--line` | `oklch(1 0 0 / 8.5%)` | `oklch(0 0 0 / 10%)` | Contorno de painéis e controles |
| `--line-strong` | `oklch(1 0 0 / 15%)` | `oklch(0 0 0 / 18%)` | Input em hover, checkbox desmarcado |
| `--fg` | `oklch(0.95 0.004 265)` | `oklch(0.22 0.012 265)` | Texto principal |
| `--fg-muted` | `oklch(0.76 0.01 265)` | `oklch(0.44 0.014 265)` | Texto secundário, ícones |
| `--fg-subtle` | `oklch(0.62 0.012 265)` | `oklch(0.54 0.014 265)` | Metadados, placeholders, `NULL` |
| `--fg-disabled` | `oklch(0.45 0.01 265)` | `oklch(0.7 0.01 265)` | Desabilitado, números de linha do editor |

Contraste sobre `--surface-1` (escuro / claro): `--fg` 15,6 / 17,1 · `--fg-muted` 8,4 / 7,7 · `--fg-subtle` 5,0 / 5,0 (mínimo 4,5).

**Camadas**, de baixo para cima: `--bg` (janela) → `--surface-1` (painéis) →
`--surface-2` (header da grade, toolbars, rodapé de diálogo) → `--surface-3` (menus,
popovers, diálogos). Uma camada nunca pula outra: popover sobre painel é `--surface-3`,
não `--surface-2`.

### 2.2 Iris — o acento

Mesma família do violeta atual, com croma reduzido de 0,23–0,27 para 0,12–0,18: a
identidade fica, a vibração sai.

| Token | Escuro | Claro | Uso |
|---|---|---|---|
| `--iris` | `oklch(0.55 0.16 282)` | `oklch(0.52 0.18 282)` | Preenchimento: botão primário, checkbox, switch (branco em cima: 5,1 / 5,9) |
| `--iris-hover` | `oklch(0.58 0.16 282)` | `oklch(0.47 0.18 282)` | Hover do preenchimento |
| `--iris-text` | `oklch(0.77 0.12 282)` | `oklch(0.5 0.18 282)` | Texto e ícone em Iris sobre painel (links) |
| `--iris-soft` | `oklch(0.66 0.15 282 / 16%)` | `oklch(0.55 0.18 282 / 10%)` | Fundo de seleção: linha, célula, texto |
| `--ring` | `oklch(0.7 0.15 282 / 60%)` | `oklch(0.55 0.18 282 / 50%)` | Anel de foco |

**Iris tem quatro papéis — e só estes:**

1. **Ação primária** — uma por área (Executar, Salvar, Conectar).
2. **Foco de teclado** — anel em `--ring`, só com `:focus-visible`.
3. **Seleção de dados** — linhas, células e texto selecionados (`--iris-soft`).
4. **Controle ligado** — checkbox, switch, cursor do editor.

**Não use Iris para:** aba ativa ou item ativo de menu/paleta (→ `--active`), coluna
ordenada (→ `--fg` + ícone de ordenação), ícone de servidor (→ cor do banco), títulos,
ícones decorativos, fundos grandes, gradientes ou brilhos.

### 2.3 Semânticas

Cada uma tem um sólido (ícone, ponto, texto) e um `-soft` (fundo). No claro os sólidos
escurecem para manter 4,5:1 sobre o painel.

| Token | Escuro | Claro | Uso |
|---|---|---|---|
| `--success` | `oklch(0.76 0.14 158)` | `oklch(0.54 0.13 158)` | Linha nova, conexão ativa, consulta concluída |
| `--success-soft` | `oklch(0.76 0.14 158 / 14%)` | `oklch(0.6 0.13 158 / 12%)` | Fundo de linha nova, toast de sucesso |
| `--warning` | `oklch(0.82 0.13 78)` | `oklch(0.56 0.12 70)` | Célula editada e não salva, aviso, ícone de PK |
| `--warning-soft` | `oklch(0.82 0.13 78 / 14%)` | `oklch(0.75 0.14 75 / 18%)` | Fundo de célula editada |
| `--danger` | `oklch(0.7 0.17 22)` | `oklch(0.55 0.19 25)` | Linha removida, erro, ação destrutiva |
| `--danger-soft` | `oklch(0.7 0.17 22 / 14%)` | `oklch(0.6 0.19 25 / 10%)` | Fundo de linha removida, botão `danger` |
| `--info` | `oklch(0.76 0.11 235)` | `oklch(0.54 0.13 240)` | Mensagens do servidor, `NOTICE`, dicas |
| `--info-soft` | `oklch(0.76 0.11 235 / 14%)` | `oklch(0.6 0.13 240 / 10%)` | Fundo de aviso informativo |

### 2.4 Alterações pendentes na grade

Codificação dupla — cor **e** forma —, legível para daltônicos. Vale para a grade, a
visão vertical e o painel de valor.

| Estado | Fundo | Gutter | Texto |
|---|---|---|---|
| Célula editada | `--warning-soft` só na célula | `•` em `--warning` | cor do tipo |
| Linha nova | `--success-soft` na linha | `+` em `--success` | cor do tipo |
| Linha removida | `--danger-soft` na linha | `−` em `--danger` | `--fg-subtle` + tachado |
| Linha selecionada | `--iris-soft` | número em `--iris-text` | — |

A seleção nunca apaga o estado pendente: numa linha pendente e selecionada, o fundo é o do
estado e a seleção aparece no gutter. Depois do `ok` do backend a tinta some em 600 ms
(`--duration-settle`, §6).

### 2.5 Identidade dos bancos

| Token | Escuro | Claro | Banco |
|---|---|---|---|
| `--db-postgres` | `oklch(0.68 0.1 250)` | `oklch(0.52 0.11 250)` | PostgreSQL |
| `--db-mongo` | `oklch(0.74 0.15 150)` | `oklch(0.55 0.14 150)` | MongoDB |
| `--db-redis` | `oklch(0.66 0.19 27)` | `oklch(0.55 0.19 27)` | Redis |
| `--db-sqlite` | `oklch(0.76 0.09 220)` | `oklch(0.56 0.1 220)` | SQLite |

Só no ícone do servidor na árvore, no ícone da aba e em badges (ponto de 6 px). Nunca como
fundo nem em texto corrido.

### 2.6 Tipos de dado na grade

| Tipo | Token | Escuro | Claro | Regra |
|---|---|---|---|---|
| numérico (`int`, `numeric`, `float`…) | `--data-number` | `oklch(0.8 0.09 235)` | `oklch(0.5 0.12 240)` | alinhado à direita, `tabular-nums` |
| texto | `--fg` | — | — | — |
| `NULL` | `--fg-subtle` | — | — | itálico, literal `NULL` |
| booleano | `--data-bool` | `oklch(0.78 0.1 350)` | `oklch(0.52 0.15 350)` | `true` / `false` normalizados |
| data / hora | `--data-date` | `oklch(0.8 0.08 185)` | `oklch(0.5 0.09 185)` | — |
| JSON / array | `--data-json` | `oklch(0.8 0.1 60)` | `oklch(0.54 0.13 55)` | — |
| uuid / binário | `--fg-muted` | — | — | — |

Croma baixo (≈ 0,09 no escuro): tingem sem gritar. A cor vem do `typeName` da coluna — a
mesma resolução que o `resolveCellEditor` já faz.

### 2.7 Sintaxe (CodeMirror)

| Token | Escuro | Claro | Uso |
|---|---|---|---|
| `--syn-keyword` | `oklch(0.76 0.12 285)` | `oklch(0.5 0.18 285)` | `SELECT`, `FROM`, `AND`, `true` |
| `--syn-function` | `oklch(0.8 0.09 235)` | `oklch(0.5 0.12 240)` | `count()`, `now()` |
| `--syn-string` | `oklch(0.8 0.1 150)` | `oklch(0.5 0.12 150)` | `'90 days'` |
| `--syn-number` | `oklch(0.8 0.1 60)` | `oklch(0.54 0.13 55)` | `50`, `12,2` |
| `--syn-type` | `oklch(0.8 0.08 185)` | `oklch(0.5 0.09 185)` | `::numeric`, `interval` |

Comentários em `--fg-subtle` itálico, operadores e pontuação em `--fg-muted`,
identificadores em `--fg`. O tema do editor é montado desses tokens (`EditorView.theme` +
`HighlightStyle`), no lugar do `oneDark` e das bordas fixas `#2d3340`/`#d0d7de`.

### 2.8 Vidro e scrim

| Token | Escuro | Claro | Uso |
|---|---|---|---|
| `--glass` | `oklch(0.26 0.009 265 / 78%)` | `oklch(1 0 0 / 80%)` | Fundo das camadas de vidro (`--surface-3` a 78% · branco a 80%) |
| `--glass-border` | `oklch(1 0 0 / 9%)` | `oklch(0 0 0 / 8%)` | Contorno do vidro |
| `--glass-highlight` | `oklch(1 0 0 / 6%)` | `oklch(1 0 0 / 70%)` | Brilho interno de 1 px no topo |
| `--scrim` | `oklch(0 0 0 / 45%)` | `oklch(0 0 0 / 25%)` | Véu sob diálogos e paleta |

### 2.9 Orçamento de cor

Numa tela típica de tabela: superfícies neutras ~70% · texto e ícones ~22% · dados e bancos
~4% · Iris ~3% · semânticas ~1%. Se Iris passar de ~5%, algo está usando o acento no lugar
de um neutro.

---

## 3. Tipografia

| Família | Uso | Pacote |
|---|---|---|
| **Geist** | Toda a interface | `@fontsource-variable/geist` |
| **Geist Mono** | Dados, SQL, tipos, atalhos | `@fontsource-variable/geist-mono` |

O app roda offline: as fontes são **empacotadas** e importadas no `globals.css`. Nada de
Google Fonts em produção. (Hoje Plus Jakarta Sans e IBM Plex Mono são declaradas mas nunca
carregadas — o que aparece é a fonte do sistema.)

| Token | Tam. / linha | Peso | Tracking | Uso |
|---|---|---|---|---|
| `text-display` | 24 / 32 | 600 | −0,02em | Estados vazios, boas-vindas |
| `text-title` | 16 / 24 | 600 | −0,01em | Título de diálogo e de painel |
| `text-heading` | 14 / 20 | 600 | 0 | Seções, cabeçalho da sidebar |
| `text-body` | 13 / 20 | 400 | 0 | Padrão: árvore, menus, abas |
| `text-label` | 13 / 20 | 500 | 0 | Botões, abas, rótulos (`text-body font-medium`) |
| `text-small` | 12 / 16 | 400 | 0 | Metadados, status bar, ajuda |
| `text-micro` | 11 / 14 | 500 | +0,04em | Rótulos de grupo, sempre em maiúsculas |
| mono dados | 12 / 16 | 400 | 0 | Células, tipos, WHERE (`font-mono text-small`) |
| mono código | 13 / 22 | 400 | 0 | Editor SQL e painel de valor |

Regras:

- **Mínimo de 11 px**, e só no `text-micro`. Nada de `text-[10px]`.
- **`tabular-nums`** em colunas numéricas, contadores, tempos e tamanhos.
- **Mono só para valores** (dados, SQL, tipos, atalhos). Rótulos sempre em Geist.
- **Hierarquia por cor primeiro** (`--fg` → `--fg-muted` → `--fg-subtle`), depois peso,
  por último tamanho. Dentro do app nada passa de 16 px, exceto `text-display`.
- Peso máximo 600; 500 em botões e abas. Maiúsculas só no `text-micro`. Itálico só em
  `NULL` e comentários.
- O componente `Typography` (h1 `4xl`, `p` com `mt-6`) é de página de conteúdo — não use em
  telas do app.

---

## 4. Espaço, tamanhos e raio

**Espaço:** `--spacing: 0.25rem` (4 px — hoje é 0,27rem, 4,32 px, e nada cai no grid).
Escala: `0.5` 2 · `1` 4 · `1.5` 6 · `2` 8 · `3` 12 · `4` 16 · `5` 20 · `6` 24 · `8` 32 ·
`10` 40 · `12` 48 · `16` 64 px. Valores arbitrários só para alinhar com algo nativo.

**Alturas de controle:**

| Tamanho | Altura | Padding x | Texto | Ícone | Uso |
|---|---|---|---|---|---|
| `xs` | 24 | 8 | 12 | 14 | Status bar, ferramentas da grade |
| `sm` | 28 | 10 | 13 | 16 | Toolbars — **padrão do app** |
| `md` | 32 | 12 | 13 | 16 | Formulários, filtros |
| `lg` | 36 | 14 | 14 | 16 | Rodapé de diálogo, CTA |

**Densidade:**

| Elemento | Altura |
|---|---|
| Linha da árvore | 28 |
| Linha da grade | 28 (compacta) · 32 (confortável) |
| Header da grade | 40 (nome + tipo) |
| Aba de documento | 30 (barra de 36) |
| Item de menu · item da paleta | 28 · 44 |
| Toolbar · status bar · cabeçalho da sidebar | 44 · 32 · 48 |
| Gutter da janela e espaço entre painéis | 8 · 6–8 |

**Raio:**

| Classe | Valor | Uso |
|---|---|---|
| `rounded-xs` | 4 px | Kbd, célula focada |
| `rounded-sm` | 6 px | Botões, inputs, linhas da árvore |
| `rounded-md` | 8 px | Abas, itens de menu/paleta |
| `rounded-lg` | 12 px | Painéis, popovers, toasts |
| `rounded-xl` | 16 px | Diálogos, paleta de comandos |
| `rounded-full` | — | Pílulas, switch, pontos |

Raio interno = raio externo − padding: botão de 6 px dentro de popover de 12 px com 6 px de
padding encaixa sem “bico”.

---

## 5. Elevação, bordas e foco

| Nível | Token | Escuro | Claro | Uso |
|---|---|---|---|---|
| 0 | — | borda `--line`, sem sombra | idem | Painéis |
| 1 | `shadow-raised` (`--elev-1`) | `0 1px 2px #00000066, inset 0 1px 0 #ffffff0a` | `0 1px 2px #1018281a, 0 0 0 1px #0000000d` | Aba ativa, thumb do segmento, botão secundário |
| 2 | `shadow-overlay` (`--elev-2`) | `0 12px 32px -8px #000000a6, 0 0 0 1px var(--line)` | `0 12px 32px -8px #10182829, 0 0 0 1px #00000014` | Menus, popovers, select, tooltip |
| 3 | `shadow-modal` (`--elev-3`) | `0 24px 64px -12px #000000bf, 0 0 0 1px var(--glass-border), inset 0 1px 0 var(--glass-highlight)` | `0 24px 64px -12px #10182840, 0 0 0 1px var(--glass-border), inset 0 1px 0 var(--glass-highlight)` | Diálogo, paleta, toast |

Níveis 0 e 1 separam por luminosidade e borda; 2 e 3 usam sombra longa e escura — no escuro
é ela que descola o menu da grade.

**Bordas:** sempre 1 px, com alpha — `--line-subtle` (linhas da grade, divisórias),
`--line` (contorno de painéis e controles), `--line-strong` (input em hover, checkbox
desmarcado).

**Foco:** um único anel por contexto, sempre em `--ring` e só com `:focus-visible`.

| Contexto | Anel |
|---|---|
| Botões e controles | 2 px, com 2 px de respiro (`--surface-1`) |
| Inputs | borda `--iris-text` + halo de 3 px em `--iris-soft` |
| Células, linhas da árvore e da grade | inset de 1,5 px, sem respiro |

---

## 6. Movimento

Curto, direcional e sempre opcional. Só `transform` e `opacity`; saídas são mais curtas
que entradas.

| Token | Valor | Uso |
|---|---|---|
| `--duration-instant` | 0 ms | Seleção, foco de célula, teclado, scroll, troca de aba |
| `--duration-press` | 80 ms | Escala do botão ao pressionar |
| `--duration-fast` | 120 ms | Hover, cor, tooltip, chevron, **todas as saídas** |
| `--duration-base` | 180 ms | Menus, popovers, select, segmento, linhas da árvore |
| `--duration-slow` | 240 ms | Diálogo, paleta, toast |
| `--duration-slower` | 320 ms | Painel de valor entrando, áreas grandes |
| `--duration-settle` | 600 ms | Tinta de alteração salva sumindo |

Os `--duration-*` são variáveis CSS comuns em `:root` (o `@theme` do Tailwind v4 não tem
namespace de duração); nos utilitários, use `duration-120`, `duration-180` e `duration-240`.

| Curva | Valor | Uso |
|---|---|---|
| `--ease-standard` | `cubic-bezier(0.2, 0, 0, 1)` | Hover, cor, press |
| `--ease-out` | `cubic-bezier(0.16, 1, 0.3, 1)` | Tudo que entra |
| `--ease-in` | `cubic-bezier(0.4, 0, 1, 1)` | Tudo que sai |
| `--ease-in-out` | `cubic-bezier(0.65, 0, 0.35, 1)` | Deslocar na tela: painel de valor, resize |
| `--ease-spring` | `cubic-bezier(0.34, 1.36, 0.64, 1)` | Indicadores pequenos: segmento, switch |

**Padrões:**

| Elemento | Entrada | Saída |
|---|---|---|
| Menu, popover, select | opacity + scale .96→1 + y −4→0 · 180 ms `out` · origem no gatilho | 120 ms `in` |
| Diálogo, paleta | scrim fade 180 ms; conteúdo scale .98→1 + y 8→0 · 240 ms `out` | 120 ms `in` |
| Toast | y 12→0 + opacity · 240 ms `out` | fade 120 ms `in` |
| Tooltip | fade + 2 px · 120 ms; atraso 500 ms, 0 ms entre vizinhos | 120 ms |
| Árvore: expandir | chevron gira 90° em 120 ms; filhos y −4→0, 180 ms, +16 ms cada (só os 10 primeiros) | instantâneo |
| Segmento, switch | thumb desliza 180 ms `spring` | — |
| Alteração salva | tinta → transparente em 600 ms, só após o `ok` | — |
| Carregando | barra indeterminada de 2 px + skeleton com brilho de 1,4 s | — |
| Hover / press | fundo e cor 120 ms `standard`; press `scale(.98)` em 80 ms | — |

**Nunca animar:** seleção e navegação por setas, scroll da grade e da árvore
(virtualizadas), troca de aba (o conteúdo aparece na hora), digitação, autocomplete e
resultado de consulta, largura/altura de listas virtualizadas. Nada em loop, exceto
indicador de carregamento. Nunca bloquear clique ou teclado esperando animação.

**Movimento reduzido** (`prefers-reduced-motion`): deslocamento e escala viram só opacidade
de 120 ms; rotações e deslizes ficam instantâneos. O loading continua (é informação).

No código, com `tw-animate-css` e os estados do Radix:

```ts
// PopoverContent / DropdownMenuContent / SelectContent
'data-[state=open]:animate-in data-[state=closed]:animate-out',
'data-[state=open]:fade-in-0 data-[state=closed]:fade-out-0',
'data-[state=open]:zoom-in-[0.96] data-[state=closed]:zoom-out-[0.96]',
'data-[side=bottom]:slide-in-from-top-1',
'duration-180 ease-out data-[state=closed]:duration-120',
'motion-reduce:zoom-in-100 motion-reduce:slide-in-from-top-0',
```

Tooltip: `TooltipProvider delayDuration={500} skipDelayDuration={0}` (hoje é `0` e pisca ao
atravessar a toolbar). Nada de `transition-all`: use `transition-colors` ou
`transition-[transform,opacity]`.

---

## 7. Vidro

Vidro é para camadas **temporárias** que passam por cima de conteúdo. Se fica aberto o
tempo todo, é superfície sólida.

| Onde usar | Receita |
|---|---|
| Paleta de comandos | `glass`, blur 24, `rounded-xl`, sobre `--scrim` |
| Toasts e aviso de atualização | `glass`, blur 20, `rounded-lg` |
| Barra flutuante de alterações pendentes | `glass`, blur 20, flutua sobre a grade |
| Menu de contexto da grade | variante leve: 92% opaco, blur 12 |

**Onde não usar:** sidebar, painéis e toolbars (fixos); header da grade e status bar;
tooltips (pequenos demais — sólidos); diálogos de formulário (leitura pede fundo sólido);
qualquer vidro sobre outro vidro.

A opacidade alta (78% no escuro, 80% no claro) mantém o texto legível sobre uma grade densa;
o blur só suaviza o que está atrás. A paleta atual (`bg-background/30` + `blur-sm`) é o
contraexemplo: a lista some sobre a grade.

```css
@utility glass {
  background: var(--glass);
  backdrop-filter: blur(20px) saturate(1.4);
  border: 1px solid var(--glass-border);
  box-shadow: var(--elev-3);
}

@media (prefers-reduced-transparency: reduce) {
  .glass { background: var(--surface-3); backdrop-filter: none; }
}
```

---

## 8. Componentes

Os primitivos continuam em `src/components/ui/` (Radix + shadcn + `cva`). Componente de
tela **não recria** botão, badge ou atalho: usa os de `components/ui`.

- **Button** — variantes `primary` (Iris, uma por área), `secondary` (neutro com borda),
  `ghost` (sem fundo, `--fg-muted`), `danger` (`--danger-soft`, sólido no hover); tamanhos
  `xs`/`sm`/`md`/`lg` (§4), padrão `sm`. Estados: hover, pressionado (`scale(.98)`), foco
  (um anel só — hoje há três receitas somadas), desabilitado (opacidade 45%) e carregando
  (spinner no lugar do ícone). Somente ícone ⇒ `aria-label` + tooltip. Atalho vai num
  `Kbd` dentro do botão.
- **Badge** — tons `neutral`, `accent`, `success`, `warning`, `danger`, `info` (fundo
  `-soft` + texto sólido), 20 px, 11 px/500; variante com ponto de 6 px (status, banco);
  `mono` para tipos (`jsonb`).
- **Kbd** — mono 11 px, 18 px de altura, `--hover` com contorno `--line-subtle`.
- **SegmentedControl** — trilho `--hover`, thumb `--surface-3` (claro: branco) com
  `shadow-raised`. Substitui os `<button>` crus da status bar (Tabela/Vertical).
- **Abas de documento** (barra de abas) — ativa sobe de camada (`--surface-1` +
  `shadow-raised` + `--fg`), inativa é transparente em `--fg-muted`, hover `--hover`.
  Sem borda tracejada e sem violeta. O ícone leva a cor do banco; `•` em `--warning` para
  alterações não salvas; fechar aparece na ativa e no hover.
- **Abas de visão** (Resultados/Mensagens) — sublinhado neutro de 2 px em `--fg`; contador
  em badge neutro.
- **Árvore** — linha de 28 px, `rounded-sm`; chevron **gira** (não troca de ícone); ícones
  em `--fg-subtle`, exceto o do servidor (cor do banco). Aberta em aba: `--active` + `--fg`.
  Foco de teclado: anel inset, não fundo. Tamanho à direita em mono 11 px `--fg-subtle`.
- **Menus, popovers, select** — sólidos: `--surface-3`, `shadow-overlay`, `rounded-lg`,
  padding 6; item de 28 px, `rounded-sm`, realce `--active` (nunca Iris); rótulo de grupo
  em `text-micro`; atalho em mono `--fg-subtle`; item destrutivo em `--danger`.
- **Tooltip** — sólido, `--surface-3` + `shadow-overlay`, 12 px, pode levar `Kbd`.
- **Toast** — `glass`, `rounded-lg`, ícone em círculo `-soft` do tom, título 13/500 +
  descrição 12 `--fg-muted`, ação opcional em botão `xs`. Sem hex fixo (hoje
  `#30D158`/`#FF375F`/`rgba(18,18,18,.5)`).
- **Diálogo** — sólido, `--surface-3`, `shadow-modal`, `rounded-xl`, título `text-title`,
  rodapé em `--surface-2` com botões `md`. Escolha do banco com o ícone na cor do banco e
  seleção por anel `--ring`.
- **Barra de alterações pendentes** (nova) — `glass` flutuando 16 px acima da status bar,
  centrada: contagem + badges (editada/nova/removida) + Descartar (`ghost`) + Salvar
  (`primary` com `⌘S`). Tira o resumo de pendências da status bar.
- **Estados de conteúdo** — vazio e erro: ícone num quadro de 44 px (`--hover`, ou
  `--danger-soft` no erro), título `text-title`, texto `--fg-muted`, ação clara (limpar
  filtro, tentar de novo, copiar erro). Carregando: barra indeterminada de 2 px no topo +
  skeleton de linhas. Nada de uma linha cinza solta (nem em inglês).
- **Iconografia** — só **Hugeicons** (substituir os `Server`/`Search` do Lucide), traço
  1,5. 16 px na interface, 14 px em controles `xs`, 18 px na busca da paleta. Cor padrão
  `--fg-muted`, `--fg` quando ativo; cor de banco só no ícone de servidor/aba.

---

## 9. No código

Tokens em [`src/styles/globals.css`](src/styles/globals.css): valores em `:root` (claro) e
`.dark` (escuro), expostos ao Tailwind v4 no `@theme inline`.

| Categoria | Utilitários |
|---|---|
| Superfícies | `bg-bg`, `bg-surface-1`, `bg-surface-2`, `bg-surface-3`, `bg-hover`, `bg-active` |
| Linhas | `border-line-subtle`, `border-line`, `border-line-strong`, `divide-line-subtle` |
| Texto | `text-fg`, `text-fg-muted`, `text-fg-subtle`, `text-fg-disabled` |
| Iris | `bg-iris`, `hover:bg-iris-hover`, `text-iris-text`, `bg-iris-soft`, `ring-ring` |
| Semânticas | `text-success`, `bg-success-soft`, … (`warning`, `danger`, `info`) |
| Bancos e dados | `text-db-postgres`, …, `text-data-number`, `text-data-json`, … |
| Tipo | `text-micro`, `text-small`, `text-body`, `text-heading`, `text-title`, `text-display`, `font-mono`, `tabular-nums` |
| Raio | `rounded-xs` … `rounded-xl` (§4) |
| Elevação | `shadow-raised`, `shadow-overlay`, `shadow-modal` |
| Movimento | `ease-standard`, `ease-out`, `ease-in`, `ease-in-out`, `ease-spring`, `duration-120/180/240` |
| Vidro | `glass`, `scrollbar-thin` |

**Aliases do shadcn.** Para os componentes atuais funcionarem no dia 1, os nomes do shadcn
apontam para o Ink: `--background → --bg`, `--foreground → --fg`, `--card → --surface-1`,
`--popover → --surface-3`, `--primary → --iris`, `--muted → --hover`,
`--muted-foreground → --fg-muted`, `--accent → --hover`, `--accent-foreground → --fg`,
`--destructive → --danger`, `--border`/`--input → --line`, `--sidebar → --surface-1`.
Atenção ao nome: no shadcn `accent` significa **hover**; o acento da marca aqui é `iris`.
Código novo usa os nomes do Ink; os aliases saem quando a migração terminar.

**Token novo:** (1) valor em `:root` **e** `.dark`; (2) `--color-*` no `@theme inline`;
(3) registre aqui. Cor usada fora do CSS (tema do CodeMirror, canvas) é lida da variável
(`var(--syn-keyword)` / `getComputedStyle`) — nunca hex em TSX.

---

## 10. Migração

Substituições mecânicas — dá para fazer por busca no projeto.

| Hoje | Ink |
|---|---|
| `bg-background` (janela) | `bg-bg` |
| `bg-sidebar` (painéis) | `bg-surface-1` |
| `bg-main` · `text-main-foreground` (não existem) | `bg-surface-1` · `text-fg` |
| `hover:bg-surface-light/50` (não existe) | `hover:bg-hover` |
| `bg-purple-glow` (não existe; status bar e log) | `bg-surface-1 border-t border-line-subtle` |
| `bg-yellow-900/30 text-yellow-200` | `bg-warning-soft` (texto segue o tipo) |
| `bg-green-900/20` · `bg-green-950/60` | `bg-success-soft` + glifo `+` |
| `bg-red-900/20` · `bg-red-950/60` | `bg-danger-soft` + `line-through` + `−` |
| `text-green-400` / `yellow-400` / `red-400` | `text-success` / `text-warning` / `text-danger` |
| `bg-primary/15` + `ring-primary` (linha) | `bg-iris-soft` |
| `ring-primary` (célula, árvore) | `ring-[1.5px] ring-inset ring-ring` |
| `bg-primary/10 text-primary` (coluna ordenada) | `text-fg` + ícone de ordenação |
| `bg-primary/12 text-primary` (paleta) | `bg-active text-fg` |
| `text-[10px]` | `text-micro` |
| `rounded` (4 px) em botões | `rounded-sm` |
| `transition-all` · `transition-color` | `transition-colors` · `transition-[transform,opacity]` |
| `#30D158` · `#FF375F` · `rgba(18,18,18,.5)` | `var(--success)` · `var(--danger)` · `glass` |
| `oneDark` + bordas `#2d3340` | tema do CodeMirror a partir de `--syn-*` e `--line` |
| `lucide-react` `Server` · `Search` | Hugeicons equivalentes |

---

## 11. Roteiro

Cada fase entrega algo visível e pode ir para a `main` sozinha.

- [ ] **Fase 0 — Consertar a base** (½ dia): instalar `tw-animate-css` (as animações já
  escritas passam a rodar); empacotar Geist e Geist Mono; `--spacing: 0.25rem`; definir ou
  remover as classes fantasmas (`bg-main`, `bg-surface-light`, `bg-purple-glow`,
  `scrollbar-thin`, `transition-color`); título da janela “Octapus”; tooltip com 500 ms.
- [ ] **Fase 1 — Tokens** (1 dia): novo `globals.css` com camadas, Iris, semânticas,
  bancos, dados, sintaxe, vidro, raios, elevação, escala de texto e curvas; aliases do
  shadcn apontando para o Ink.
- [ ] **Fase 2 — Primitivos** (1–2 dias): Button, Badge, Kbd, SegmentedControl, Tabs
  (documento e visão), Input; Menu/Select/Popover/Tooltip com as specs de movimento.
- [ ] **Fase 3 — Superfícies** (2–3 dias): shell, sidebar e árvore, barra de abas,
  toolbar com breadcrumb, status bar; grade com cor por tipo, números à direita e gutter com
  `•` `+` `−`; painel de valor; tema do CodeMirror a partir dos tokens.
- [ ] **Fase 4 — Vidro e movimento** (1–2 dias): utilitário `glass` na paleta, toasts e
  barra de alterações; expandir da árvore; skeleton e barra de progresso; movimento e
  transparência reduzidos.
- [ ] **Fase 5 — Tema claro e QA** (1 dia): paridade claro/escuro em todos os estados;
  contraste AA conferido no `pnpm dev:mock`; atualizar o status no topo deste documento.

---

## 12. Checklist de PR

1. Nenhuma cor crua em TSX: nada de `bg-red-*`, `text-white/60`, `#hex` ou `rgba` — só
   tokens.
2. Iris só nos quatro papéis (§2.2).
3. Estado novo nasce em par claro/escuro e passa 4,5:1 (3:1 para ícones e bordas de
   controle).
4. Animar só `transform` e `opacity`, com 120/180/240 ms e as curvas do tema; nada de
   `transition-all`.
5. Vidro só via `glass`, e só em camadas flutuantes (§7).
6. Texto mínimo de 11 px; `tabular-nums` em números; valores em mono.
7. Um conjunto de ícones (Hugeicons), 16 px, traço 1,5.
8. Componente de tela usa `Button`/`Badge`/`Kbd` de `components/ui` — não recria.
