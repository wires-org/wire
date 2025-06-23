{
  wire.testing.test_remote_deploy = {
    nodes.deployer = {
      _wire.deployer = true;
    };
    nodes.receiver = {
      _wire.receiver = true;
    };
    testScript = ''
      with subtest("Test unreachable hosts"):
        deployer.fail(f"wire apply --on receiver-unreachable --no-progress --path {TEST_DIR}/hive.nix --no-keys -vvv >&2")

      with subtest("Check basic apply"):
          deployer.succeed(f"wire apply --on receiver --no-progress --path {TEST_DIR}/hive.nix --no-keys -vvv >&2")

          identity = receiver.succeed("cat /etc/identity")
          assert identity == "first", "Identity of first apply wasn't as expected"

      with subtest("Check boot apply"):
        first_system = receiver.succeed("readlink -f /run/current-system")

        deployer.succeed(f"wire apply boot --on receiver-second --no-progress --path {TEST_DIR}/hive.nix --no-keys -vvv >&2")

        _first_system = receiver.succeed("cat /etc/identity")
        assert first_system == _first_system, "apply boot without --rebot changed /run/current-system"

      with subtest("Check /etc/identity after reboot"):
        receiver.reboot()

        identity = receiver.succeed("cat /etc/identity")
        assert identity == "second", "Identity didn't change after second apply"

      # with subtest("Check --reboot"):
      #   deployer.succeed(f"wire apply boot --on receiver-third --no-progress --path {TEST_DIR}/hive.nix --reboot --no-keys -vvv >&2")
      #
      #   identity = receiver.succeed("cat /etc/identity")
      #   assert identity == "third", "Identity didn't change after third apply"
    '';
  };
}
