{
  perSystem =
    {
      pkgs,
      self',
      ...
    }:
    {
      packages.docs = pkgs.callPackage ./package.nix {
        inherit (self'.packages) wire-small wire-dignostics-md;
      };
    };
}
