{
  hive,
  path,
  nixosConfigurations ? { },
  nixpkgs ? null,
}:
let
  module = import ./module.nix;

  mergedHive = {
    meta = { };

    defaults = { };
  } // hive;

  nodeNames = builtins.filter (
    name:
    !builtins.elem name [
      "meta"
      "defaults"
    ]
  ) (builtins.attrNames mergedHive);

  resolvedNixpkgs =
    if mergedHive.meta ? "nixpkgs" then
      # support '<nixpkgs>' and 'import <nixpkgs> {}'
      if builtins.isPath mergedHive.meta.nixpkgs then
        import mergedHive.meta.nixpkgs { }
      else
        mergedHive.meta.nixpkgs
    else
      import nixpkgs { };

  filtedNixosConfigurations = nixpkgs.lib.filterAttrs (
    name: _v: mergedHive ? name
  ) nixosConfigurations;

  evaluateNode =
    name:
    let
      evalConfig = import (resolvedNixpkgs.path + "/nixos/lib/eval-config.nix");
      hive =
        mergedHive
        // (builtins.mapAttrs (name: value: {
          imports = value._module.args.modules ++ [ hive.${name} or { } ];
        }) filtedNixosConfigurations);
    in
    evalConfig {
      modules = [
        module

        hive.defaults
        hive.${name}
      ];
      system = null;
      specialArgs = {
        inherit name nodes;
      } // hive.meta.specialArgs or { };
    };
  nodes = builtins.listToAttrs (
    map (name: {
      inherit name;
      value = evaluateNode name;
    }) nodeNames
  );

  getTopLevel = node: (evaluateNode node).config.system.build.toplevel.drvPath;
in
rec {
  inherit evaluateNode getTopLevel nodes;

  inspect = {
    inherit path;
    nodes = builtins.mapAttrs (_: v: v.config.deployment) nodes;
  };
}
