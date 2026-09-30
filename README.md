# octapus-db — empacotamento Nix

Branch só de empacotamento do [octapus-db](https://github.com/LKTeloeken/octapus_db).
Não tem o código do app: reempacota o `.deb` oficial de cada release para rodar no
NixOS. O código e o ambiente de desenvolvimento (`nix develop`) estão na `main`.

O binário publicado é ligado dinamicamente contra caminhos do FHS (`/usr/lib/...`),
que não existem no NixOS. O `nix/package.nix` reescreve o interpretador e as libs para
o `/nix/store` com `autoPatchelfHook` e injeta o que o WebKit precisa em runtime.

## Usar

Rodar sem instalar:
```bash
nix run github:LKTeloeken/octapus_db/nix
```

Instalar no perfil:
```bash
nix profile install github:LKTeloeken/octapus_db/nix
```

Em configuração declarativa, use o pacote direto ou o overlay (`overlays.default`,
que expõe `pkgs.octapus-db`):
```nix
{
  inputs.octapus-db.url = "github:LKTeloeken/octapus_db/nix";

  # no módulo do sistema:
  environment.systemPackages = [ inputs.octapus-db.packages.x86_64-linux.default ];
}
```

Atualizar: `nix profile upgrade --all --refresh`, ou `nix flake update octapus-db` na
sua configuração. O auto-update embutido do app não funciona aqui: o `/nix/store` é
somente-leitura e o updater do Tauri no Linux só atualiza AppImage.

## Como esta branch é mantida

- `nix/release.nix` (versão + hash do `.deb`) é regerado pelo workflow
  `.github/workflows/nix-release.yml` da `main` sempre que uma release é **publicada**.
  Ele só grava aqui depois que `nix build` passar com o hash novo.
- Para refazer à mão: `./nix/update-release.sh [versão]` (precisa de `gh` e `jq`; usa o
  `nix` se houver, senão `openssl`), ou dispare o workflow em *Actions → Nix → Run
  workflow*.
- Só há bundle para **x86_64-linux**, o único Linux que o pipeline publica. Para arm64,
  acrescente a linha em `nix/update-release.sh`.
- Mudanças de empacotamento (dependência nova, `.desktop`, wrapper) se fazem aqui, em
  `nix/package.nix`.
