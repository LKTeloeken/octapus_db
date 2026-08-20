#!/usr/bin/env bash
# Regenera nix/release.nix apontando para uma release publicada no GitHub.
#
#   ./nix/update-release.sh                 # usa a última release
#   ./nix/update-release.sh 0.1.0-beta.16   # usa uma versão específica
#
# Precisa do `gh` autenticado, do `nix` (para o hash SRI) e do `jq`.
set -euo pipefail

repo="LKTeloeken/octapus_db"
raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Sistema do Nix -> sufixo de arquitetura no .deb que o tauri-action publica.
# Ao passar a publicar Linux arm64 no pipeline, basta acrescentar a linha aqui.
bundles=("x86_64-linux:amd64")

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
    hash="$(nix --extra-experimental-features nix-command \
      store prefetch-file --json "$url" | jq -r .hash)"

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
