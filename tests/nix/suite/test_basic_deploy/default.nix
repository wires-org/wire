{
  wire.testing.test_basic_deploy = {
    nodes.deployer = {
      _wire.deployer = true;
    };
    nodes.receiver = {
      _wire.receiver = true;
    };
    testScript = ''
      deployer.succeed(f"wire apply --on receiver --no-progress --path {TEST_DIR}/hive.nix --no-keys -vvv >&2")
      receiver.succeed("test -f /etc/a")
    '';
  };
}
