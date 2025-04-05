{
  inputs = {
    # ...
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    wire.url = "github:wires-org/wire"; # [!code ++]
  };

  outputs = {
    # ...
    nixpkgs,
    home-manager,
    wire, # [!code ++]
    ...
  }: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
  in {
    homeConfigurations.my-user = home-manager.lib.homeManagerConfiguration {
      inherit pkgs;
      modules = [
        # ...
        {
          home.packages = [
            wire.packages.${system}.wire # [!code ++]
          ];
        }
      ];
    };
  };
}
