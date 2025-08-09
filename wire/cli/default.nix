{ getSystem, inputs, ... }:
{
  perSystem =
    {
      pkgs,
      lib,
      self',
      buildRustProgram,
      system,
      ...
    }:
    let
      cleanSystem = system: lib.replaceStrings [ "-" ] [ "_" ] system;
      agents = lib.strings.concatMapStrings (
        system: "--set WIRE_KEY_AGENT_${cleanSystem system} ${(getSystem system).packages.agent} "
      ) (import inputs.linux-systems);
    in
    {
      packages = {
        default = self'.packages.wire;
        wire-unwrapped = buildRustProgram {
          name = "wire";
          pname = "wire";
          cargoExtraArgs = "-p wire";
          doCheck = true;
          nativeBuildInputs = [ pkgs.installShellFiles ];
          postInstall = ''
            installShellCompletion --cmd wire \
                --bash <($out/bin/wire completions bash) \
                --fish <($out/bin/wire completions fish) \
                --zsh <($out/bin/wire completions zsh)
          '';
        };

        wire = pkgs.symlinkJoin {
          name = "wire";
          paths = [ self'.packages.wire-unwrapped ];
          nativeBuildInputs = [
            pkgs.makeWrapper
          ];
          postBuild = ''
            wrapProgram $out/bin/wire ${agents}
          '';
          meta.mainProgram = "wire";
        };

        wire-small = pkgs.symlinkJoin {
          name = "wire";
          paths = [ self'.packages.wire-unwrapped ];
          nativeBuildInputs = [
            pkgs.makeWrapper
          ];
          postBuild = ''
            wrapProgram $out/bin/wire --set WIRE_KEY_AGENT_${cleanSystem system} ${self'.packages.agent}
          '';
          meta.mainProgram = "wire";
        };

        wire-dignostics-md = self'.packages.wire-unwrapped.overrideAttrs {
          DIAGNOSTICS_MD_OUTPUT = "./diagnostics";
          preBuild = ''
            mkdir -p ./diagnostics
          '';
          installPhase = ''
            mv ./diagnostics/DIAGNOSTICS.md $out
          '';
        };
      };
    };
}
