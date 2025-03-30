{
  lib,
  snakeOil,
  wire,
  config,
  ...
}:
let
  inherit (lib)
    mkEnableOption
    mkMerge
    mkIf
    ;
  cfg = config._wire;
in
{
  options._wire = {
    deployer = mkEnableOption "deployment-specific settings";
    receiver = mkEnableOption "receiver-specific settings";
  };

  config = mkMerge [
    (mkIf cfg.deployer {
      systemd.tmpfiles.rules = [
        "L+ /root/.ssh/id_ed25519 - - - - ${snakeOil.snakeOilEd25519PrivateKey}"
      ];
      environment.systemPackages = [ wire ];
    })
    (mkIf cfg.receiver {
      services.openssh.enable = true;
      users.users.root.openssh.authorizedKeys.keys = [ snakeOil.snakeOilEd25519PublicKey ];
    })
  ];
}
