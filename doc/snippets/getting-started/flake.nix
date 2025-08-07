{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.wire.url = "github:wires-org/wire";

  outputs = inputs @ {
    nixpkgs,
    wire,
    ...
  }: {
    wire = wire.makeHive {
      meta = {
        nixpkgs = import nixpkgs {
          system = "x86_64-linux";
        };
        specialArgs = {
          inherit inputs;
        };
      };

      defaults = {
        # ...
      };

      node-a = {
        # ...
      };
    };
  };
}
