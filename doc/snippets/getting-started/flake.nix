{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = _: {
    colmena = {
      defaults = {
        # ...
      };

      node-a = {
        # ...
      };
    };
  };
}
