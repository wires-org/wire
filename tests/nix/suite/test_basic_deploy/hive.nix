let
  mkHiveNode = import ../utils.nix { testName = "test_basic_deploy-@IDENT@"; };
in
{
  meta.nixpkgs = import <nixpkgs> { system = "x86_64-linux"; };
  receiver = mkHiveNode { hostname = "receiver"; } {
    environment.etc."a".text = "b";
  };
}
