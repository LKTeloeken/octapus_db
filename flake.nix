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
      # Congelado junto com o pacote (ver `avisoMudanca`); o overlay atualizado
      # está na branch `nix`.
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
        # O pacote mudou para a branch `nix` (github:LKTeloeken/octapus_db/nix),
        # atualizada sozinha a cada release. Aqui ele fica congelado na última
        # versão pinada, só para não quebrar quem instalou pelo endereço antigo —
        # com um aviso a cada avaliação. Remover numa versão futura.
        temBundle = release.bundles ? ${system};
        avisoMudanca = pkgs.lib.warn (
          "octapus-db: o pacote Nix mudou para github:LKTeloeken/octapus_db/nix; "
          + "este endereço está congelado na ${release.version} e não recebe mais versões."
        );
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
          octapus-db = avisoMudanca (pkgs.callPackage ./nix/package.nix { });
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
