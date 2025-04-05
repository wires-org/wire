# Getting Started

## Installation

Wire can be heavy to compile. You should enable the subsitutor `wires.cachix.org`.

::: code-group

<<< @/snippets/getting-started/cache.nix [module.nix]
<<< @/snippets/getting-started/nix.conf

:::

### NixOS / Home Manager

::: code-group

<<< @/snippets/getting-started/nixos.flake.nix [flake.nix (NixOS)]
<<< @/snippets/getting-started/hm.flake.nix [flake.nix (Home Manager)]
<<< @/snippets/getting-started/configuration.nix
<<< @/snippets/getting-started/home.nix

:::
