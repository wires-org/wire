---
comment: true
title: Getting Started
description: Getting started with Wire Tool!
---

# {{ $frontmatter.title }}

## Installation

Wire can be heavy to compile. You should enable the substituter `wires.cachix.org`.

::: code-group

<<< @/snippets/getting-started/cache.nix [module.nix]
<<< @/snippets/getting-started/nix.conf

:::

### Supported Nix & NixOS versions

Wire is currently _tested_ against `unstable`, `24.11` and `24.05`.
For each channel, it is tested `24.11`'s `pkgs.nix` and against the given channel's `pkgs.lix`.

It will be tested against every channel's stable nix once
[#126](https://github.com/wires-org/wire/issues/126) is solved.

### NixOS / Home Manager

::: code-group

<<< @/snippets/getting-started/nixos.flake.nix [flake.nix (NixOS)]
<<< @/snippets/getting-started/hm.flake.nix [flake.nix (Home Manager)]
<<< @/snippets/getting-started/configuration.nix
<<< @/snippets/getting-started/home.nix

:::
