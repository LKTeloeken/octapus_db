# Congelado: o empacotamento Nix mudou para a branch `nix`, onde este arquivo é
# regerado a cada release publicada. Fica aqui só para o endereço antigo do flake
# continuar instalando a última versão pinada.
{
  version = "1.0.0";

  bundles = {
    x86_64-linux = {
      arch = "amd64";
      hash = "sha256-okbzjyXr1r1CaZs699BZ258dLApQ6oEgGWjbm5OElm4=";
    };
  };
}
