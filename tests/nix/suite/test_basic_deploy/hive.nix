let
  utils = import ../utils.nix { testName = "test_basic_deploy"; };
in
{
  meta.nixpkgs = <nixpkgs>;
  receiver = utils.mkHiveNode "x86_64-linux" "receiver" {
    environment.etc."a".text = "b";
  };
}
