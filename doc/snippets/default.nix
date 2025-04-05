{
  getting-started-hm = import ./getting-started/home.nix;
  getting-started-hm-flake = import ./getting-started/hm.flake.nix;
  getting-started-nixos = import ./getting-started/configuration.nix;
  getting-started-nixos-flake = import ./getting-started/nixos.flake.nix;
  getting-started-cache = import ./getting-started/cache.nix;
}
