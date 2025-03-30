let
  utils = import ../utils.nix;
in
{
  meta.nixpkgs = <nixpkgs>;
  receiver = utils.popTest {
    networking.hostName = "receiverb";
  };
}
