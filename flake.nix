{
  description = "octapus-db — cliente de banco de dados desktop (Tauri 2 + React)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils }:
    let
      release = import ./nix/release.nix;
    in
    {
      # Para consumir o pacote em configurações NixOS / home-manager sem precisar
      # referenciar `packages.<system>` na mão.
      overlays.default = final: _prev: {
        octapus-db = final.callPackage ./nix/package.nix { };
      };
    }
    // utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        libraries = with pkgs; [
          webkitgtk_4_1
          gtk3
          cairo
          gdk-pixbuf
          glib
          pango
          harfbuzz
          librsvg
          openssl
        ];
        # Só os sistemas com bundle publicado ganham `packages`/`apps`; nos demais
        # (macOS, aarch64-linux) o flake continua servindo só o devShell.
        temBundle = release.bundles ? ${system};
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = libraries;
          nativeBuildInputs = with pkgs; [
            pkg-config
            gobject-introspection
            cargo
            rustc
            pnpm
          ];
          shellHook = ''
            export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath libraries}:$LD_LIBRARY_PATH
          '';
        };
      }
      // pkgs.lib.optionalAttrs temBundle (
        let
          octapus-db = pkgs.callPackage ./nix/package.nix { };
        in
        {
          packages = {
            default = octapus-db;
            inherit octapus-db;
          };

          apps.default = {
            type = "app";
            program = "${octapus-db}/bin/octapus_db";
            meta.description = "Roda o octapus-db";
          };
        }
      ));
}
