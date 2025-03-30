let
  utils = import ../utils.nix;
in
{
  receiver = utils.popTest {
    networking.hostName = "receiverb";
  };
}
