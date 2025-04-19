{
  perSystem =
    {
      pkgs,
      self',
      buildRustProgram,
      ...
    }:
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
            wrapProgram $out/bin/wire \
                --set WIRE_RUNTIME ${../../runtime} \
                --set WIRE_AGENT ${self'.packages.agent}
          '';
          meta.mainProgram = "wire";
        };
      };
    };
}
