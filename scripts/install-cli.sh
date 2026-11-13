#!/bin/sh
# Instala o `octapus` de terminal sem o app: baixa o binário da release
# (a mais nova, ou a de OCTAPUS_VERSION), confere o SHA-256 e grava em
# ~/.local/bin (ou em OCTAPUS_INSTALL_DIR).
#
#   curl -fsSL https://raw.githubusercontent.com/LKTeloeken/octapus_db/main/scripts/install-cli.sh | sh
#   OCTAPUS_VERSION=1.2.0 sh install-cli.sh
#
# Quem tem o app instala pelo botão das Preferências (Terminal), que baixa
# sempre a versão do próprio app. No Windows, use o botão do app.
set -eu

repo="LKTeloeken/octapus_db"
dir="${OCTAPUS_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) asset="octapus-macos-arm64" ;;
  Darwin-x86_64) asset="octapus-macos-x64" ;;
  Linux-x86_64) asset="octapus-linux-x86_64" ;;
  *)
    echo "erro: não há octapus publicado para $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

if [ -n "${OCTAPUS_VERSION:-}" ]; then
  base="https://github.com/$repo/releases/download/app-v$OCTAPUS_VERSION"
else
  base="https://github.com/$repo/releases/latest/download"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "baixando $asset…"
curl -fsSL "$base/$asset" -o "$tmp/octapus"
curl -fsSL "$base/$asset.sha256" -o "$tmp/octapus.sha256"

expected="$(cut -d ' ' -f 1 < "$tmp/octapus.sha256")"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/octapus" | cut -d ' ' -f 1)"
else
  actual="$(shasum -a 256 "$tmp/octapus" | cut -d ' ' -f 1)"
fi
if [ "$expected" != "$actual" ]; then
  echo "erro: o arquivo baixado não confere com o SHA-256 da release; nada foi instalado" >&2
  exit 1
fi

mkdir -p "$dir"
chmod 755 "$tmp/octapus"
mv "$tmp/octapus" "$dir/octapus"
echo "octapus instalado em $dir/octapus"

case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "acrescente $dir ao PATH do seu shell para chamar só \"octapus\"" ;;
esac
