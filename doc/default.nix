{
  pkgs,
  catppuccin,
  lib,
  nixosOptionsDoc,
  runCommand,
  wire,
  ...
}: let
  eval = lib.evalModules {
    modules = [
      ../lib/src/module.nix
      {
        options._module.args = lib.mkOption {
          internal = true;
        };
      }
    ];
    specialArgs = {
      name = "‹node name›";
      nodes = {};
    };
  };

  optionsMd =
    (nixosOptionsDoc {
      inherit (eval) options;
    })
    .optionsCommonMark;

  optionsDoc = runCommand "options-doc.md" {} ''
    cat ${optionsMd} > $out
  '';
in
  pkgs.stdenv.mkDerivation {
    name = "wire-docs";
    buildInputs = with pkgs; [mdbook catppuccin.packages.${pkgs.system}.default];
    src = ./.;
    buildPhase = ''
      cat ${optionsDoc} >> ./src/modules/README.md
      ${lib.getExe wire} inspect --markdown-help > ./src/cli/README.md
      mdbook build -d $out
    '';
  }
