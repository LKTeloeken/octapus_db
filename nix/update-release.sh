#!/usr/bin/env bash
# Regenera nix/release.nix apontando para uma release publicada no GitHub.
#
#   ./nix/update-release.sh                 # usa a última release
#   ./nix/update-release.sh 1.0.0           # usa uma versão específica
#
# Roda sozinho no workflow `nix-release.yml` (na main) quando uma release é
# publicada; à mão, só para corrigir ou refazer. Precisa do `gh` autenticado e
# do `jq`. O hash SRI sai do `nix` quando ele existe; sem ele (ex.: macOS), do
# `openssl` — os dois dão o mesmo `sha256-<base64>`.
set -euo pipefail

repo="LKTeloeken/octapus_db"
raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Sistema do Nix -> sufixo de arquitetura no .deb que o tauri-action publica.
# Ao passar a publicar Linux arm64 no pipeline, basta acrescentar a linha aqui.
bundles=("x86_64-linux:amd64")

hash_sri() {
  if command -v nix >/dev/null 2>&1; then
    nix --extra-experimental-features nix-command \
      store prefetch-file --json "$1" | jq -r .hash
  else
    echo "sha256-$(curl -fsSL "$1" | openssl dgst -sha256 -binary | base64)"
  fi
}

version="${1:-}"
if [ -z "$version" ]; then
  tag="$(gh release view --repo "$repo" --json tagName -q .tagName)"
  version="${tag#app-v}"
fi

saida="$(
  cat <<EOF
# Metadados do .deb publicado no GitHub Releases que nix/package.nix reempacota.
# Gerado por nix/update-release.sh — não edite à mão.
{
  version = "$version";

  bundles = {
EOF

  for entrada in "${bundles[@]}"; do
    sistema="${entrada%%:*}"
    arch="${entrada##*:}"
    url="https://github.com/$repo/releases/download/app-v$version/octapus-db_${version}_${arch}.deb"

    echo "baixando $url" >&2
    hash="$(hash_sri "$url")"

    cat <<EOF
    $sistema = {
      arch = "$arch";
      hash = "$hash";
    };
EOF
  done

  cat <<'EOF'
  };
}
EOF
)"

printf '%s\n' "$saida" > "$raiz/nix/release.nix"
echo "nix/release.nix atualizado para $version" >&2
