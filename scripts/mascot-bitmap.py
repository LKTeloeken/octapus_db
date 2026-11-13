#!/usr/bin/env python3
"""Rasteriza o logo (src/assets/octapus.svg) num mapa de pontos para o polvo da
TUI (crates/octapus-cli/src/tui/mascot.txt): cada célula do terminal mostra
2x4 pontos em braille. Rodar de novo só se o logo mudar:

    python3 scripts/mascot-bitmap.py
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SVG = ROOT / "src/assets/octapus.svg"
OUT = ROOT / "crates/octapus-cli/src/tui/mascot.txt"
# (nome, largura em pontos): a altura sai da proporção do logo
SIZES = [("large", 56), ("small", 28)]


def subpaths(d):
    """Os contornos do path (só M e C, absolutos), com as curvas em segmentos."""
    tokens = re.findall(r"[MC]|-?\d+\.?\d*(?:e-?\d+)?", d)
    paths, current, command, i = [], [], None, 0
    while i < len(tokens):
        if tokens[i] in "MC":
            command = tokens[i]
            i += 1
            continue
        if command == "M":
            if current:
                paths.append(current)
            current = [(float(tokens[i]), float(tokens[i + 1]))]
            i += 2
            command = "C"
        else:
            p0 = current[-1]
            p1, p2, p3 = [(float(tokens[i + k]), float(tokens[i + k + 1])) for k in (0, 2, 4)]
            i += 6
            for step in range(1, 13):
                t = step / 12
                u = 1 - t
                current.append(tuple(
                    u**3 * p0[axis] + 3 * u * u * t * p1[axis] + 3 * u * t * t * p2[axis] + t**3 * p3[axis]
                    for axis in (0, 1)
                ))
    if current:
        paths.append(current)
    return paths


def inside(paths, x, y):
    """Regra evenodd: os olhos são furos no contorno."""
    crossings = 0
    for path in paths:
        for (x0, y0), (x1, y1) in zip(path, path[1:] + path[:1]):
            if (y0 > y) != (y1 > y) and x < x0 + (y - y0) * (x1 - x0) / (y1 - y0):
                crossings += 1
    return crossings % 2 == 1


def main():
    svg = SVG.read_text()
    width, height = map(float, re.search(r'viewBox="0 0 ([\d.]+) ([\d.]+)"', svg).groups())
    paths = subpaths(re.search(r' d="([^"]+)"', svg).group(1))

    blocks = []
    for name, columns in SIZES:
        rows = round(columns * height / width / 4) * 4
        lines = []
        for row in range(rows):
            line = ""
            for column in range(columns):
                # 3x3 amostras por ponto: aceso se a maioria cai dentro
                hits = sum(
                    inside(paths, (column + (sx + 0.5) / 3) * width / columns, (row + (sy + 0.5) / 3) * height / rows)
                    for sx in range(3)
                    for sy in range(3)
                )
                line += "#" if hits >= 5 else "."
            lines.append(line)
        blocks.append(f"@{name}\n" + "\n".join(lines))
    OUT.write_text("\n".join(blocks) + "\n")
    print(f"{OUT.relative_to(ROOT)}: " + ", ".join(f"{name} {columns} pontos" for name, columns in SIZES))


if __name__ == "__main__":
    main()
