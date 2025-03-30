{ config, ... }:
{
  wire.testing.test_basic_deploy = {
    nodes.deployer = {
      _wire.deployer = true;
    };
    nodes.receiver = {
      _wire.receiver = true;
    };
    testScript = ''
      receiver.wait_for_unit("multi-user.target")
      receiver.wait_for_unit("sshd.service")
      deployer.succeed("wire --help >&2")
      deployer.succeed("wire apply --on receiver --no-progress --path ${config.wire.testing.test_basic_deploy.testDir}/hive.nix --no-keys -vvv >&2")
      receiver.succeed("test $(cat /etc/hostname) == \"receiverb\"")

    '';
  };
}
