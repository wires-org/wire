let
  inherit (import ../../..) makeHive;
in
makeHive {
  meta = {
    nixpkgs = <nixpkgs>;
  };

  node-a = {
    deployment._keys = [
      {
        name = "different-than-a";
        source = "hi";
      }
    ];

    nixpkgs.hostPlatform = "x86_64-linux";
  };
}
