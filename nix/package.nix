# Reempacota o .deb oficial da release para rodar no NixOS.
#
# O binário do Tauri é ligado dinamicamente contra caminhos do FHS
# (`/lib64/ld-linux-x86-64.so.2`, `/usr/lib/...`), que não existem aqui: o
# `autoPatchelfHook` reescreve o interpretador e as libs para o /nix/store, e o
# `wrapGAppsHook3` injeta os schemas do GSettings e os módulos GIO que o WebKit
# precisa em runtime.
{
  lib,
  stdenv,
  fetchurl,
  autoPatchelfHook,
  dpkg,
  wrapGAppsHook3,
  cairo,
  dbus,
  gdk-pixbuf,
  glib,
  glib-networking,
  gtk3,
  libsoup_3,
  openssl,
  webkitgtk_4_1,
}:

let
  release = import ./release.nix;
  bundle =
    release.bundles.${stdenv.hostPlatform.system}
      or (throw "octapus-db: não há bundle publicado para ${stdenv.hostPlatform.system}");
in
stdenv.mkDerivation {
  pname = "octapus-db";
  inherit (release) version;

  src = fetchurl {
    url = "https://github.com/LKTeloeken/octapus_db/releases/download/app-v${release.version}/octapus-db_${release.version}_${bundle.arch}.deb";
    inherit (bundle) hash;
  };

  nativeBuildInputs = [
    autoPatchelfHook
    dpkg
    wrapGAppsHook3
  ];

  # As libs que o binário lista em `readelf -d`, mais o glib-networking — que não
  # é ligado, mas é o backend de TLS do libsoup/WebKit (sem ele, nenhum https).
  buildInputs = [
    cairo
    dbus
    gdk-pixbuf
    glib
    glib-networking
    gtk3
    libsoup_3
    openssl
    stdenv.cc.cc.lib # libgcc_s.so.1
    webkitgtk_4_1
  ];

  unpackPhase = ''
    runHook preUnpack
    dpkg-deb -x "$src" .
    runHook postUnpack
  '';

  installPhase = ''
    runHook preInstall

    install -Dm755 usr/bin/octapus_db "$out/bin/octapus_db"

    mkdir -p "$out/share"
    cp -r usr/share/applications usr/share/icons "$out/share/"

    # O .desktop do bundler sai sem categoria e com Exec relativo ao PATH.
    substituteInPlace "$out/share/applications/octapus-db.desktop" \
      --replace-fail "Exec=octapus_db" "Exec=$out/bin/octapus_db" \
      --replace-fail "Categories=" "Categories=Development;Database;"
    echo "StartupWMClass=octapus_db" >> "$out/share/applications/octapus-db.desktop"

    runHook postInstall
  '';

  # Sem isso o WebKit costuma abrir uma janela em branco em boa parte das
  # GPUs/compositores; `--set-default` deixa quem quiser reativar o dmabuf.
  preFixup = ''
    gappsWrapperArgs+=(--set-default WEBKIT_DISABLE_DMABUF_RENDERER 1)
  '';

  # Alias com hífen (o nome do produto), já apontando para o binário embrulhado.
  postFixup = ''
    ln -s "$out/bin/octapus_db" "$out/bin/octapus-db"
  '';

  meta = {
    description = "Cliente de banco de dados desktop para PostgreSQL, MongoDB e Redis";
    homepage = "https://github.com/LKTeloeken/octapus_db";
    mainProgram = "octapus_db";
    platforms = builtins.attrNames release.bundles;
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
  };
}
