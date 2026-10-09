#!/usr/bin/env python3
"""Resume perf/catalog/results/*.jsonl numa tabela por (suite, tier, rede).

Uso: summarize.py [filtro-de-tier]   — a última medição de cada caso vence.
"""
import json
import pathlib
import sys

results = pathlib.Path(__file__).parent / "results"
tier_filter = sys.argv[1] if len(sys.argv) > 1 else None

latest = {}
for path in sorted(results.glob("*.jsonl")):
    for line in path.read_text().splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        if tier_filter and row["tier"] != tier_filter:
            continue
        extra = row["extra"]
        # Casos repetidos com parâmetros diferentes (ex.: um schema por linha)
        discriminator = extra.get("schema", "")
        key = (row["suite"], row["tier"], row["net"], row["case"], discriminator)
        latest[key] = row

groups = {}
for key, row in latest.items():
    groups.setdefault(key[:3], []).append(row)

for (suite, tier, net), rows in sorted(groups.items()):
    print(f"\n## {suite} · tier {tier} · {net}\n")
    print(f"| caso | ms | detalhes |\n|---|---:|---|")
    for row in sorted(rows, key=lambda r: r["at"]):
        ms = "—" if row["ms"] is None else f"{row['ms']:,.1f}"
        extra = ", ".join(f"{k}={v}" for k, v in row["extra"].items())
        print(f"| {row['case']} | {ms} | {extra} |")
