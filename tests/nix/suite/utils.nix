let
  lock = builtins.fromJSON (builtins.readFile ../../../flake.lock);
  nodeName = lock.nodes.root.inputs.nixpkgs;
  lockedNode = lock.nodes.${nodeName}.locked;
  nixpkgs = import (fetchTarball {
    url = lockedNode.url or "https://github.com/NixOS/nixpkgs/archive/${lockedNode.rev}.tar.gz";
    sha256 = lockedNode.narHash;
  }) { };

  createDonor =
    pkgs:
    pkgs.lib.evalTest {
      imports = [
        {
          nodes.donor = { };
          hostPkgs = pkgs;
        }
      ];
    };
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
        let
          snakeOil = import "${pkgs.path}/nixos/tests/ssh-keys.nix" pkgs;
          donor = createDonor pkgs;
        in
        {
          imports = [
            "${modulesPath}/virtualisation/disk-image.nix"
            (nixpkgs.path + "/nixos/lib/testing/nixos-test-base.nix")
            donor.config.nodes.donor.system.build.networkConfig
          ];

          nix = {
            nixPath = [ "nixpkgs=${pkgs.path}" ];
            settings.substituters = lib.mkForce [ ];
          };

          virtualisation.memorySize = 4096;
          system.switch.enable = true;

          services.openssh.enable = true;
          users.users.root.openssh.authorizedKeys.keys = [ snakeOil.snakeOilEd25519PublicKey ];
        }
      )
    ];
  };
}
