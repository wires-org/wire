{
  self,
  config,
  lib,
  inputs,
  ...
}:
let
  inherit (lib)
    mkOption
    mapAttrs'
    mapAttrsToList
    flatten
    ;
  inherit (lib.types)
    submodule
    lines
    attrsOf
    deferredModule
    lazyAttrsOf
    ;
  cfg = config.wire.testing;
in
{
  imports = [ ./suite/test_basic_deploy ];
  options.wire.testing = mkOption {
    type = attrsOf (
      submodule (
        { name, ... }:
        {
          options = {
            nodes = mkOption {
              type = lazyAttrsOf deferredModule;
            };
            testScript = mkOption {
              type = lines;
              default = ''

              '';
              description = "test script for runNixOSTest";
            };
            testDir = mkOption {
              default = "${self}/tests/nix/suite/${name}";
              readOnly = true;
            };
          };
          config = {
            testScript = ''
              start_all()
            '';
          };
        }
      )
    );
    description = "A set of test cases for wire VM testing suite";
  };

  config.perSystem =
    {
      pkgs,
      self',
      ...
    }:
    {
      checks = mapAttrs' (testName: opts: {
        name = "nixos-vm-test-${testName}";
        value = pkgs.testers.runNixOSTest {
          inherit (opts) nodes;
          name = testName;
          defaults =
            {
              pkgs,
              evaluateHive,
              testDir,
              ...
            }:
            let

              hive = evaluateHive {
                nixpkgs = pkgs.path;
                path = testDir;
                hive = builtins.scopedImport {
                  __nixPath = _b: null;
                  __findFile = path: name: if name == "nixpkgs" then pkgs.path else throw "oops!!";
                } "${testDir}/hive.nix";
              };
              nodes = mapAttrsToList (_: val: val.config.system.build.toplevel.drvPath) hive.nodes;
              # fetch **all** dependencies of a flake
              # it's called fetchLayer because my naming skills are awful
              fetchLayer =
                input:
                let
                  subLayers = if input ? inputs then map fetchLayer (builtins.attrValues input.inputs) else [ ];
                in
                [
                  input.outPath
                ]
                ++ subLayers;
            in
            {
              imports = [ ./test-opts.nix ];
              nix = {
                nixPath = [ "nixpkgs=${pkgs.path}" ];
                settings.substituters = lib.mkForce [ ];
              };

              virtualisation.memorySize = 4096;

              virtualisation.additionalPaths = flatten (
                nodes
                ++ (mapAttrsToList (_: fetchLayer) inputs)
                ++ [
                  self'.packages.wire
                  self'.packages.agent
                ]
              );

            };
          node.specialArgs = {
            evaluateHive = import "${self}/runtime/evaluate.nix";
            suiteDir = "${self}/tests/nix";
            inherit testName;
            snakeOil = import "${pkgs.path}/nixos/tests/ssh-keys.nix" pkgs;
            inherit (opts) testDir;
            inherit (self'.packages) wire;
          };
          inherit (opts) testScript;
        };
      }) cfg;
    };
}
