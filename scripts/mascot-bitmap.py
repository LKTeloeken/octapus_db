#!/usr/bin/env python3
"""Desenha o logo (src/assets/octapus.svg) em caracteres de bloco para o polvo
da TUI (crates/octapus-cli/src/tui/mascot.txt): cada célula do terminal mostra
2x2 pontos (os quadrantes ▘▝▖▗…). Saem dois quadros, o de olho aberto e o da
piscada. Rodar de novo só se o logo mudar:

    python3 scripts/mascot-bitmap.py
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SVG = ROOT / "src/assets/octapus.svg"
OUT = ROOT / "crates/octapus-cli/src/tui/mascot.txt"
# Largura em células; a altura sai da proporção do logo
COLUMNS = 16
# Os 16 quadrantes: bit 1 = cima-esquerda, 2 = cima-direita, 4 = baixo-esquerda, 8 = baixo-direita
QUADRANTS = " ▘▝▀▖▌▞▛▗▚▐▜▄▙▟█"


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


def crossings(paths, x, y):
    """Quantos contornos têm o ponto dentro (um raio para a direita, por contorno)."""
    inside = 0
    for path in paths:
        count = 0
        for (x0, y0), (x1, y1) in zip(path, path[1:] + path[:1]):
            if (y0 > y) != (y1 > y) and x < x0 + (y - y0) * (x1 - x0) / (y1 - y0):
                count += 1
        inside += count % 2
    return inside


def frame(paths, width, height, columns, lines, lit):
    """O logo em `columns` x `lines` células; `lit` decide, por quantos contornos contêm o ponto, se ele acende."""
    dots = [
        [
            # 3x3 amostras por ponto: aceso se a maioria cai dentro
            sum(
                lit(crossings(paths, (column + (sx + 0.5) / 3) * width / (columns * 2), (row + (sy + 0.5) / 3) * height / (lines * 2)))
                for sx in range(3)
                for sy in range(3)
            )
            >= 5
            for column in range(columns * 2)
        ]
        for row in range(lines * 2)
    ]
    return [
        "".join(
            QUADRANTS[dots[2 * line][2 * cell] + 2 * dots[2 * line][2 * cell + 1] + 4 * dots[2 * line + 1][2 * cell] + 8 * dots[2 * line + 1][2 * cell + 1]]
            for cell in range(columns)
        ).rstrip()
        for line in range(lines)
    ]


def main():
    svg = SVG.read_text()
    width, height = map(float, re.search(r'viewBox="0 0 ([\d.]+) ([\d.]+)"', svg).groups())
    paths = subpaths(re.search(r' d="([^"]+)"', svg).group(1))
    lines = round(COLUMNS * height / width / 2)

    frames = [
        # Dentro de um contorno só: o olho, que é um contorno dentro do outro, fica furado
        ("open", frame(paths, width, height, COLUMNS, lines, lambda inside: inside % 2 == 1)),
        # A piscada: o furo do olho fechado
        ("blink", frame(paths, width, height, COLUMNS, lines, lambda inside: inside > 0)),
    ]
    OUT.write_text("".join(f"@{name}\n" + "\n".join(art) + "\n" for name, art in frames))
    print(f"{OUT.relative_to(ROOT)}: {COLUMNS} x {lines} células, {len(frames)} quadros")


if __name__ == "__main__":
    main()
