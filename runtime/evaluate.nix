{
  hive,
  path,
  nixosConfigurations ? {},
  nixpkgs ? null,
}: let
  module = import ./module.nix;

  mergedHive =
    {
      meta = {};

      defaults = {};
    }
    // hive
    # Map nixosConfigurations into nodes
    // (builtins.mapAttrs (name: value: {
        imports =
          value._module.args.modules
          # Include any custom stuff within `colmena`
          ++ [hive.${name} or {}];
      })
      nixosConfigurations);

  nodeNames = with builtins; filter (name: !elem name ["meta" "defaults"]) (attrNames mergedHive);

  resolvedNixpkgs =
    if mergedHive.meta ? "nixpkgs"
    then
      # support '<nixpkgs>' and 'import <nixpkgs> {}'
      if builtins.isPath mergedHive.meta.nixpkgs
      then import mergedHive.meta.nixpkgs {}
      else mergedHive.meta.nixpkgs
    else import nixpkgs {};

  evaluateNode = name: let
    evalConfig = import (resolvedNixpkgs.path + "/nixos/lib/eval-config.nix");
  in
    evalConfig {
      modules = [
        module

        mergedHive.defaults
        mergedHive.${name}
      ];

      specialArgs =
        {
          inherit name;
          nodes = nodeNames;
        }
        // mergedHive.meta.specialArgs or {};
    };

  getTopLevel = node: (evaluateNode node).config.system.build.toplevel.drvPath;
in rec {
  inherit evaluateNode getTopLevel;

  nodes =
    builtins.listToAttrs
    (map (name: {
        inherit name;
        value = evaluateNode name;
      })
      nodeNames);

  inspect = {
    inherit path;
    nodes = builtins.mapAttrs (_: v: v.config.deployment) nodes;
  };
}
