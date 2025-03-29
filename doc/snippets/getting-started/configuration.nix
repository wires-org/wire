{ system, ... }:
let
  wire = import (
    # [!code ++]
    builtins.fetchTarball "https://github.com/wires-org/wire/archive/refs/heads/main.tar.gz" # [!code ++]
  ); # [!code ++]
in
{
  environment.systemPackages = [
    wire.packages.${system}.wire # [!code ++]
  ];

  # ...
}
