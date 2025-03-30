let
  lock = builtins.fromJSON (builtins.readFile ../../../flake.lock);
  nodeName = lock.nodes.root.inputs.nixpkgs;
  lockedNode = lock.nodes.${nodeName}.locked;
  nixpkgs = fetchTarball {
    url = lockedNode.url or "https://github.com/NixOS/nixpkgs/archive/${lockedNode.rev}.tar.gz";
    sha256 = lockedNode.narHash;
  };
in
{
  popTest = cfg: {
    imports = [
      cfg
      (
        {
          modulesPath,
          ...
        }:
        {
          imports = [ "${modulesPath}/virtualisation/disk-image.nix" ];
        }
      )
    ];
  };
}
