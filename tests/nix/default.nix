{
  self,
  config,
  lib,
  ...
}:
let
  inherit (lib) mkOption mapAttrs' mapAttrsToList;
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
    type = attrsOf (submodule {
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
      };
      config = {
        testScript = ''
          start_all()
        '';
      };
    });
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
                hive = import "${testDir}/hive.nix";
              };
              nodes = mapAttrsToList (_: val: val.config.system.build.toplevel.drvPath) hive.nodes;
            in
            {
              imports = [ ./test-opts.nix ];
              nix = {
                nixPath = [ "nixpkgs=${pkgs.path}" ];
                settings.substituters = lib.mkForce [ ];
              };

              virtualisation.additionalPaths = nodes;

            };
          node.specialArgs = {
            evaluateHive = import "${self}/runtime/evaluate.nix";
            suiteDir = "${self}/tests/nix";
            inherit testName;
            testDir = "${self}/tests/nix/suite/${testName}";
            snakeOil = import "${pkgs.path}/nixos/tests/ssh-keys.nix" pkgs;
            inherit (self'.packages) wire;
          };
          inherit (opts) testScript;
        };
      }) cfg;
    };
}
