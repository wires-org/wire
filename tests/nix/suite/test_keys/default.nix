{
  wire.testing.test_keys = {
    nodes.deployer = {
      _wire.deployer = true;
      _wire.receiver = true;
    };
    nodes.receiver = {
      _wire.receiver = true;
    };
    testScript = ''
      deployer_so = collect_store_objects(deployer)
      receiver_so = collect_store_objects(receiver)

      # build all nodes without any keys
      deployer.succeed(f"wire apply --no-progress --on receiver --path {TEST_DIR}/hive.nix --no-keys --ssh-accept-host -vvv >&2")

      receiver.wait_for_unit("sshd.service")

      # --no-keys should never push a key
      receiver.fail("test -f /run/keys/source_string")
      deployer.fail("test -f /run/keys/source_string")

      def test_keys(target, target_object):
          deployer.succeed(f"wire apply keys --on {target} --no-progress --non-interactive --path {TEST_DIR}/hive.nix --ssh-accept-host -vvv >&2")

          keys = [
            ("/run/keys/source_string", "hello_world_source", "root root 600"),
            ("/etc/keys/file", "hello_world_file", "root root 644"),
            ("/home/owner/some/deep/path/command", "hello_world_command", "owner owner 644"),
            ("/run/keys/environment", "string_from_environment", "root root 600"),
          ]

          for path, value, permissions in keys:
              # test existence & value
              source_string = target_object.succeed(f"cat {path}")
              assert value in source_string, f"{path} has correct contents ({target})"

              stat = target_object.succeed(f"stat -c '%U %G %a' {path}").rstrip()
              assert permissions == stat, f"{path} has correct permissions ({target})"

      def perform_routine(target, target_object):
          test_keys(target, target_object)

          # Mess with the keys to make sure that every push refreshes the permissions
          target_object.succeed("echo 'incorrect_value' > /run/keys/source_string")
          target_object.succeed("chown 600 /etc/keys/file")
          # Test having a key that doesn't exist mixed with keys that do
          target_object.succeed("rm /home/owner/some/deep/path/command")

          # Test keys twice to ensure the operation is idempotent,
          # especially around directory creation.
          test_keys(target, target_object)

      perform_routine("receiver", receiver)
      perform_routine("deployer", deployer)

      new_deployer_store_objects = collect_store_objects(deployer).difference(deployer_so)
      new_receiver_store_objects = collect_store_objects(receiver).difference(receiver_so)

      # no one should have any keys introduced by the operation
      for node, objects in [
        (deployer, new_deployer_store_objects),
        (receiver, new_receiver_store_objects),
      ]:
        assert_store_not_posioned(node, "hello_world_source", objects)
        assert_store_not_posioned(node, "hello_world_file", objects)
        assert_store_not_posioned(node, "hello_world_command", objects)
        assert_store_not_posioned(node, "string_from_environment", objects)
    '';
  };
}
