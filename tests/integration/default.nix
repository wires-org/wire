{
  flake,
  wire ? (import "${flake}").flake.outputs.packages.x86_64-linux.wire,
  pkgs ? (import ./nixpkgs.nix),
  ...
}:
let
  inherit (pkgs) lib;
  sshKeys = import (pkgs.path + "/nixos/tests/ssh-keys.nix") pkgs;

  commonModule =
    { pkgs, ... }:
    {
      nix.nixPath = [ "nixpkgs=${pkgs.path}" ];
      nix.settings.substituters = lib.mkForce [ ];
      virtualisation = {
        memorySize = lib.mkForce (1024 * 5);
        writableStore = true;
        additionalPaths = [
          pkgs.path
          (getPrebuiltNode "node")
          (import "${flake}").tarball
          (import "${flake}").flake.inputs.nixpkgs.outPath
          ./..
        ];
      };

      services.openssh.enable = true;
      users.users.root.openssh.authorizedKeys.keys = [
        sshKeys.snakeOilPublicKey
      ];

      boot.loader.grub.enable = false;
    };

  deployerModule =
    { pkgs, ... }:
    {
      imports = [ commonModule ];
      environment.systemPackages = [
        wire
        pkgs.git
        (pkgs.writeShellScriptBin "run-copy-stderr" ''
          exec "$@" 2>&1
        '')
      ];
    };

  targetModule =
    { ... }:
    {
      imports = [ commonModule ];
      system.switch.enable = true;
    };

  nodes = {
    deployer = deployerModule;
    node = targetModule;
  };

  evalTest =
    module:
    pkgs.testers.runNixOSTest {
      inherit nodes;
      name = "deployer";

      imports = [
        module
        # commonModule
      ];
    };

  evaluate = import "${flake}/runtime/evaluate.nix";
  getPrebuiltNode =
    name:
    (evaluate {
      hive = import ./hive.nix;
      path = ./.;
      nixosConfigurations = { };
      nixpkgs = pkgs;
    }).getTopLevel
      name;
in
evalTest (
  { pkgs, ... }:
  {
    testScript = _: ''
      start_all()

      deployer.succeed("nix-store -qR ${getPrebuiltNode "node"}")
      node.succeed("nix-store -qR ${getPrebuiltNode "node"}")
      deployer.succeed("nix-store -qR ${pkgs.path}")
      node.succeed("nix-store -qR ${pkgs.path}")
      deployer.succeed("ln -sf ${pkgs.path} /nixpkgs")
      node.succeed("ln -sf ${pkgs.path} /nixpkgs")

      node.wait_for_unit("sshd.service")

      # Make deployer use ssh snake oil
      deployer.succeed("mkdir -p /root/.ssh && touch /root/.ssh/id_rsa && chmod 0600 /root/.ssh/id_rsa && cat ${sshKeys.snakeOilPrivateKey} > /root/.ssh/id_rsa")

      deployer.wait_until_succeeds("ssh -o StrictHostKeyChecking=accept-new node true", timeout=30)

      deployer.succeed("wire apply switch --no-progress -vv --no-keys --path ${flake}/tests/integration")

      # node.succeed("stat /etc/post-switch")
    '';
  }
)
