let
  flake = import ../../../.;
in
{
  popTest = cfg: {
    imports = [
      cfg
      "${flake.inputs.nixpkgs}/nixos/lib/testing/nixos-test-base.nix"
    ];
  };
}
