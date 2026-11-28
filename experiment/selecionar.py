#!/usr/bin/env python3
"""Seleção do conjunto de perguntas do experimento, a partir do catálogo.

    python3 experiment/selecionar.py [--total 200]

Lê `experiment/data/catalog.json` (o `./experiment/x catalog` monta e confere)
e grava:

- `experiment/perguntas-todas.csv`: as 1.450 perguntas do catálogo, com a
  situação de cada uma (elegível ou o motivo de ficar de fora), a categoria, a
  pontuação de relevância e se foi selecionada;
- `experiment/selecao.json`: as selecionadas, com o texto em inglês e os
  gabaritos, para montar o `perguntas.toml` com o texto em português.

Elegível: os gabaritos do PostgreSQL e do MongoDB rodaram duas vezes com o
mesmo resultado, os dois resultados são iguais e não vazios. O Redis tem os
dados do PostgreSQL (`redis-load`), então a pergunta vale nos três.

Relevância (maior primeiro), para cada elegível:
- +2 se a dica `evidence` do BIRD não traz fórmula (divisão, porcentagem,
  soma...): o assistente não recebe a dica, e a pergunta que depende dela
  mede a fórmula escondida, não a tradução;
- +1 se o resultado tem até 10 linhas (o veredito fica fácil de conferir).

A escolha é gulosa e determinística: a cada passo entra a pergunta de maior
relevância entre as do estrato (banco × dificuldade × categoria) mais longe
da sua cota, com o id do BIRD como desempate. As cotas são proporcionais ao
que há de elegível, com um mínimo por banco, para os 11 bancos aparecerem.
"""

import argparse
import csv
import json
import re
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
FORMULA = re.compile(r"DIVIDE|SUBTRACT|MULTIPLY|SUM\(|COUNT\(|AVG|MAX\(|MIN\(|percent|[*/]\s*\d|\bdivid", re.I)

# Da mais complexa para a mais simples: a categoria é o primeiro traço que a
# consulta de referência tem
CATEGORIES = [
    ("agrupamento", lambda u: "GROUP BY" in u),
    ("cálculo", lambda u: "CAST(" in u or "/" in u or "*" in u.replace("COUNT(*)", "")),
    ("agregação", lambda u: re.search(r"\b(COUNT|SUM|AVG|MAX|MIN)\(", u) is not None),
    ("ordenação", lambda u: "ORDER BY" in u),
    ("junção", lambda u: " JOIN " in u),
    ("consulta simples", lambda u: True),
]


def category(sql: str) -> str:
    upper = " ".join(sql.upper().split())
    return next(name for name, test in CATEGORIES if test(upper))


def status(entry: dict) -> str:
    verified = entry.get("verified") or {}
    if set(entry.get("gold", {})) - set(entry.get("problems", {})) != {"postgres", "mongodb"}:
        return "sem gabarito dos dois paradigmas"
    if verified.get("problems"):
        return "gabarito com problema na execução"
    if verified.get("agree") is False:
        return "gabaritos dão resultados diferentes"
    if verified.get("agree") is not True:
        return "gabaritos não conferidos"
    if verified["rows"].get("postgres", 0) == 0:
        return "resultado vazio"
    return "elegível"


def number(entry: dict) -> int:
    return int(entry["source"].split(":")[1])


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--total", type=int, default=200)
    parser.add_argument("--min-per-dataset", type=int, default=12)
    args = parser.parse_args()

    entries = json.loads((HERE / "data" / "catalog.json").read_text())["entries"]
    for entry in entries:
        entry["status"] = status(entry)
        sql = entry.get("gold", {}).get("postgres", "")
        entry["category"] = category(sql) if sql else ""
        rows = (entry.get("verified") or {}).get("rows", {}).get("postgres")
        entry["formula"] = bool(FORMULA.search(entry.get("evidence") or ""))
        entry["score"] = (0 if entry["formula"] else 2) + (1 if rows is not None and rows <= 10 else 0)

    pool = [entry for entry in entries if entry["status"] == "elegível"]
    if len(pool) < args.total:
        raise SystemExit(f"só há {len(pool)} elegíveis")

    # Cota por banco: proporcional, com um mínimo; e, dentro do banco, por
    # dificuldade e categoria na proporção do que há
    by_dataset = Counter(entry["dataset"] for entry in pool)
    quota = {name: min(count, args.min_per_dataset) for name, count in by_dataset.items()}
    rest = args.total - sum(quota.values())
    spare = {name: by_dataset[name] - quota[name] for name in by_dataset}
    spare_total = sum(spare.values())
    for name in sorted(by_dataset):
        quota[name] += round(rest * spare[name] / spare_total)
    # O arredondamento pode errar por uma ou duas
    order = sorted(by_dataset, key=lambda name: -spare[name])
    while sum(quota.values()) != args.total:
        step = 1 if sum(quota.values()) < args.total else -1
        for name in order:
            if sum(quota.values()) == args.total:
                break
            if 0 < quota[name] + step <= by_dataset[name]:
                quota[name] += step

    def key(entry):
        return (entry["dataset"], entry["difficulty"], entry["category"])

    stratum_size = Counter(key(entry) for entry in pool)
    selected = []
    taken = Counter()
    left = sorted(pool, key=lambda entry: (-entry["score"], number(entry)))
    for dataset, target in sorted(quota.items()):
        mine = [entry for entry in left if entry["dataset"] == dataset]
        size = by_dataset[dataset]
        for _ in range(target):
            # O estrato mais atrás da sua parte proporcional; nele, a mais relevante
            def lag(entry):
                share = stratum_size[key(entry)] / size * target
                return taken[key(entry)] - share

            best = min(mine, key=lambda entry: (round(lag(entry), 6), -entry["score"], number(entry)))
            mine.remove(best)
            taken[key(best)] += 1
            selected.append(best)
    chosen = {entry["source"] for entry in selected}

    with (HERE / "perguntas-todas.csv").open("w", newline="") as file:
        writer = csv.writer(file)
        writer.writerow(
            ["fonte", "banco", "dificuldade", "categoria", "situacao", "linhas_gabarito",
             "dica_com_formula", "relevancia", "selecionada", "pergunta_en"]
        )
        for entry in sorted(entries, key=lambda entry: (entry["dataset"], number(entry))):
            rows = (entry.get("verified") or {}).get("rows", {}).get("postgres", "")
            writer.writerow([
                entry["source"], entry["dataset"], entry.get("difficulty") or "", entry["category"],
                entry["status"], rows, "sim" if entry["formula"] else "não",
                entry["score"] if entry["status"] == "elegível" else "",
                "sim" if entry["source"] in chosen else "não", entry["question"],
            ])

    selected.sort(key=lambda entry: (entry["dataset"], number(entry)))
    (HERE / "selecao.json").write_text(json.dumps([
        {
            "source": entry["source"], "dataset": entry["dataset"], "difficulty": entry["difficulty"],
            "category": entry["category"], "en": entry["question"],
            "postgres": entry["gold"]["postgres"], "mongodb": entry["gold"]["mongodb"],
        }
        for entry in selected
    ], ensure_ascii=False, indent=1))

    print(f"{len(entries)} no catálogo, {len(pool)} elegíveis, {len(selected)} selecionadas")
    print("situação:", dict(Counter(entry["status"] for entry in entries)))
    print("por banco:", dict(sorted(Counter(entry["dataset"] for entry in selected).items())))
    print("por dificuldade:", dict(Counter(entry["difficulty"] for entry in selected)))
    print("por categoria:", dict(Counter(entry["category"] for entry in selected)))
    print("relevância:", dict(sorted(Counter(entry["score"] for entry in selected).items())))


if __name__ == "__main__":
    main()
