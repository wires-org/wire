let
  inherit (import ../utils.nix { testName = "test_keys-@IDENT@"; }) makeHive mkHiveNode;
in
makeHive {
  meta.nixpkgs = import <nixpkgs> { system = "x86_64-linux"; };

  receiver = mkHiveNode { hostname = "receiver"; } {
    environment.etc."identity".text = "first";

    # test node pinging
    deployment.target.hosts = [
      "unreachable-1"
      "unreachable-2"
      "unreachable-3"
      "unreachable-4"
      "receiver"
    ];
  };

  receiver-second = mkHiveNode { hostname = "receiver"; } {
    environment.etc."identity".text = "second";
    deployment.target.host = "receiver";
  };

  receiver-third = mkHiveNode { hostname = "receiver"; } {
    environment.etc."identity".text = "third";
    deployment.target.host = "receiver";
  };

  receiver-unreachable = mkHiveNode { hostname = "receiver"; } {
    # test node pinging
    deployment.target.hosts = [
      "completely-unreachable"
    ];
  };
}
