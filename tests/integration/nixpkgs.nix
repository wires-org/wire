let nixpkgs = (import ../default.nix).flake.inputs.nixpkgs.outPath; in import nixpkgs {}
