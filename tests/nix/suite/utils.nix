let
  lock = builtins.fromJSON (builtins.readFile ../../../flake.lock);
  nodeName = lock.nodes.root.inputs.nixpkgs;
  lockedNode = lock.nodes.${nodeName}.locked;
  nixpkgs = import (fetchTarball {
    url = lockedNode.url or "https://github.com/NixOS/nixpkgs/archive/${lockedNode.rev}.tar.gz";
    sha256 = lockedNode.narHash;
  }) { };
in
{
  inherit nixpkgs;
  popTest = cfg: {
    imports = [
      cfg
      (
        {
          modulesPath,
          pkgs,
          lib,
          ...
        }:
        {
          imports = [
            "${modulesPath}/virtualisation/disk-image.nix"
            (nixpkgs.path + "/nixos/lib/testing/nixos-test-base.nix")
          ];

          nix = {
            nixPath = [ "nixpkgs=${pkgs.path}" ];
            settings.substituters = lib.mkForce [ ];
          };

          virtualisation.memorySize = 4096;
          system.switch.enable = true;
        }
      )
    ];
  };
}
