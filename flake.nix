{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-compat.url = "github:edolstra/flake-compat";
    git-hooks.url = "github:cachix/git-hooks.nix";
    systems.url = "github:nix-systems/default";
    crane.url = "github:ipetkov/crane";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };
  outputs =
    {
      flake-parts,
      systems,
      git-hooks,
      crane,
      treefmt-nix,
      ...
    }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        git-hooks.flakeModule
        treefmt-nix.flakeModule
        ./nix/hooks.nix # pre-commit hooks
        ./nix/utils.nix # utility functions
        ./nix/shells.nix
        ./nix/checks.nix
        ./wire/cli
        ./wire/key_agent
        ./doc
      ];
      systems = import systems;

      perSystem =
        {
          pkgs,
          inputs',
          config,
          lib,
          ...
        }:
        {
          _module.args = {
            toolchain = inputs'.fenix.packages.complete;
            craneLib = (crane.mkLib pkgs).overrideToolchain config._module.args.toolchain.toolchain;
          };
          treefmt = {
            programs = {
              # rfc style
              nixfmt.enable = true;
              # docs only
              alejandra.enable = true;
              rustfmt.enable = true;
              just.enable = true;
              prettier.enable = true;
              protolint.enable = true;
              taplo.enable = true;
            };
            settings.formatter = {
              nixfmt.excludes = [ "doc/snippets/*.nix" ];
              alejandra = {
                includes = lib.mkForce [ "doc/snippets/*.nix" ];
              };
            };
          };
        };
    };
}
