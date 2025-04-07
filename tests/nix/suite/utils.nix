{ testName }:
let
  flake = import ../../../.;
in
{
  popTest = system: name: cfg: {
    imports = [
      cfg
      (
        {
          modulesPath,
          ...
        }:
        {
          imports = [
            "${modulesPath}/virtualisation/qemu-vm.nix"
            "${modulesPath}/testing/test-instrumentation.nix"
            flake.checks.${system}."nixos-vm-test-${testName}".nodes.${name}.system.build.networkConfig
          ];

          boot.loader.grub.enable = false;
        }
      )
    ];
  };
}
