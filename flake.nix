{
  description = "octapus-db — cliente de banco de dados desktop (Tauri 2 + React)";

  # Branch só de empacotamento: não tem o código do app, só reempacota o .deb
  # publicado nas releases. O `nix/release.nix` é atualizado pelo workflow
  # `nix-release.yml` (na main) a cada release publicada.

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
    # Só os sistemas com bundle publicado (hoje, x86_64-linux).
    // utils.lib.eachSystem (builtins.attrNames release.bundles) (system:
      let
        pkgs = import nixpkgs { inherit system; };
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
      });
}
