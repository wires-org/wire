{
  meta.nixpkgs = import ./nixpkgs.nix;

  node = {
    pkgs,
    lib,
    ...
  }: let
    sshKeys = import (pkgs.path + "/nixos/tests/ssh-keys.nix") pkgs;
  in {
    deployment.target.host = "node";
    deployment.buildOnTarget = false;

    nix.nixPath = ["nixpkgs=/nixpkgs"];
    nix.settings.substituters = lib.mkForce [];
    virtualisation = {
      memorySize = lib.mkForce (1024 * 5);
      writableStore = true;
      additionalPaths = [pkgs.path];
    };

    services.openssh.enable = true;
    users.users.root.openssh.authorizedKeys.keys = [
      sshKeys.snakeOilPublicKey
    ];

    system.switch.enable = true;

    imports = let
      # WTF is this and why does it work?
      pkgs = import ./nixpkgs.nix;
    in [
      (pkgs.path + "/nixos/lib/testing/nixos-test-base.nix")
    ];

    boot.loader.grub.enable = false;

    environment.etc."post-switch" = {
      text = "exists";
    };
  };
}
