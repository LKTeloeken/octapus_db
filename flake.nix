{
  description = "Tauri development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils }:
    utils.lib.eachDefaultSystem (system:
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
      });
}
