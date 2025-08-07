{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.wire.url = "github:wires-org/wire";

  outputs = {
    self,
    nixpkgs,
    wire,
    ...
  } @ inputs: {
    wire = wire.makeHive {
      # Give wire our ninixosConfigurations
      inherit (self) nixosConfigurations;

      meta = {
        # ... from above
      };

      node-a.deployment = {
        # ...
      };
    };

    nixosConfigurations = {
      node-a = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = {inherit inputs;};
        modules = [
          {
            nixpkgs.hostPlatform = "x86_64-linux";
          }
        ];
      };

      node-b = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = {inherit inputs;};
        modules = [
          {
            nixpkgs.hostPlatform = "x86_64-linux";
          }
        ];
      };
    };
  };
}
