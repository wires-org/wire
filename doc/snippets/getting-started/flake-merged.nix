{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = {nixpkgs, ...} @ inputs: {
    colmena = {
      node-a = {
        deployment = {
          target = {
            # ...
          };

          keys = {
            # ...
          };
        };
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
    };
  };
}
