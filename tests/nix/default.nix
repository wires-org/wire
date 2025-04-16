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
    mapAttrsToList
    flatten
    cartesianProduct
    ;
  inherit (lib.types)
    submodule
    lines
    attrsOf
    anything
    lazyAttrsOf
    ;
  cfg = config.wire.testing;
in
{
  imports = [
    ./suite/test_basic_deploy
    ./suite/test_local_deploy
  ];
  options.wire.testing = mkOption {
    type = attrsOf (
      submodule (_: {
        options = {
          nodes = mkOption {
            type = lazyAttrsOf anything;
          };
          testScript = mkOption {
            type = lines;
            default = '''';
            description = "test script for runNixOSTest";
          };
        };
      })
    );
    description = "A set of test cases for wire VM testing suite";
  };

  config.perSystem =
    {
      pkgs,
      self',
      inputs',
      ...
    }:
    let
      nixNixpkgsCombos = cartesianProduct {
        nixpkgs = [
          inputs'.nixpkgs
          inputs'.nixpkgs_current_stable
          inputs'.nixpkgs_prev_stable
        ];
        nix = [
          "nix"
          "lix"
        ];
        testName = builtins.attrNames cfg;
      };
      mkTest =
        {
          testName,
          opts,
          nix,
          nixpkgs,
        }:
        let
          # NOTE: nix is pinned to current_stable until #126 is solved
          nixPackage =
            if nix == "lix" then
              nixpkgs.legacyPackages.lix
            else
              inputs'.nixpkgs_current_stable.legacyPackages.nix;
          sanitizeName =
            str: lib.strings.sanitizeDerivationName (builtins.replaceStrings [ "." ] [ "_" ] str);
          identifier = sanitizeName "${nixpkgs.legacyPackages.lib.trivial.release}-${nixPackage.name}";
          path = "tests/nix/suite/${testName}";
          injectedFlakeDir = pkgs.runCommand "injected-flake-dir" { } ''
            cp -r ${../..} $out
            chmod -R +w $out
            substituteInPlace $out/${path}/hive.nix --replace @IDENT@ ${identifier}
          '';
        in
        rec {
          name = "nixos-vm-test-${testName}-${identifier}";
          value = nixpkgs.legacyPackages.testers.runNixOSTest {
            inherit (opts) nodes;
            inherit name;
            defaults =
              {
                pkgs,
                evaluateHive,
                ...
              }:
              let
                hive = evaluateHive {
                  nixpkgs = pkgs.path;
                  path = injectedFlakeDir;
                  hive = builtins.scopedImport {
                    __nixPath = _b: null;
                    __findFile = path: name: if name == "nixpkgs" then pkgs.path else throw "oops!!";
                  } "${injectedFlakeDir}/${path}/hive.nix";
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
                  package = nixPackage;
                  nixPath = [ "nixpkgs=${pkgs.path}" ];
                  settings.substituters = lib.mkForce [ ];
                };

                virtualisation.memorySize = 4096;
                virtualisation.additionalPaths = flatten [
                  injectedFlakeDir
                  nodes
                  (mapAttrsToList (_: fetchLayer) inputs)
                ];
              };
            node.specialArgs = {
              evaluateHive = import "${self}/runtime/evaluate.nix";
              testName = name;
              snakeOil = import "${pkgs.path}/nixos/tests/ssh-keys.nix" pkgs;
              inherit (self'.packages) wire;
            };
            # NOTE: there is surely a better way of doing this in a more
            # "controlled" manner, but until a need is asked for, this will remain
            # as is.
            testScript =
              ''
                start_all()

                TEST_DIR="${injectedFlakeDir}/${path}"
              ''
              + lib.concatStringsSep "\n" (mapAttrsToList (_: value: value._wire.testScript) value.nodes)
              + opts.testScript;
          };
        };
    in
    {
      checks = builtins.listToAttrs (
        builtins.map (
          {
            nix,
            nixpkgs,
            testName,
          }:
          let
            opts = cfg.${testName};
          in
          mkTest {
            inherit
              testName
              opts
              nix
              nixpkgs
              ;
          }
        ) nixNixpkgsCombos
      );
    };
}
