let
  utils = import ../utils.nix;
in
{
  meta.nixpkgs = utils.nixpkgs;
  receiver = utils.popTest {
    networking.hostName = "receiverb";
  };
}
