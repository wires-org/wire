{ testName }:
let
  # Use the flake-compat code in project root to access the tests which are
  # defined through Flakes, as flake-parts is heavily depended on here.
  flake = import ../../../.;
in
{
  # This is glue for the newly deployed VMs as they need specific configuration
  # such as static network configuration and other nitpicky VM-specific options.
  # I thank Colmena & NixOps devs for providing me pointers on how to correctly create this, so
  # thank you to those who made them!
  #
  mkHiveNode =
    {
      hostname,
      system ? "x86_64-linux",
    }:
    cfg: {
      imports = [
        cfg
        (
          { modulesPath, ... }:
          {
            imports = [
              "${modulesPath}/virtualisation/qemu-vm.nix"
              "${modulesPath}/testing/test-instrumentation.nix"
              flake.checks.${system}."nixos-vm-test-${testName}".nodes.${hostname}.system.build.networkConfig
            ];

            nixpkgs.hostPlatform = system;
            boot.loader.grub.enable = false;
          }
        )
      ];
    };

  __functor = self: self.mkHiveNode;
}
