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

### NixOS / Home Manager

::: code-group

<<< @/snippets/getting-started/nixos.flake.nix [flake.nix (NixOS)]
<<< @/snippets/getting-started/hm.flake.nix [flake.nix (Home Manager)]
<<< @/snippets/getting-started/configuration.nix
<<< @/snippets/getting-started/home.nix

:::

## Your First Hive

Wire groups your machines into _nodes_, which are NixOS configurations with
additional information for deployment. Start by creating a `hive.nix` in the same directory as your
`configuration.nix`.

A `hive.nix` is an attribute set with NixOS configurations, each with a unique
name. Add a node for your local machine:

```nix:line-numbers [hive.nix]
{
  meta.nixpkgs = import <nixpkgs> {};

  my-local-machine = {
    imports = [./configuration.nix];

    # If you don't know, find this value by running
    # `nix eval --expr 'builtins.currentSystem' --impure`
    nixpkgs.hostPlatform = "x86_64-linux";
  };
}
```

Now, assuming your host machine is currently `my-local-machine`, simply running
[`wire apply`](/reference/cli.html#wire-apply) will evaluate, build, and
activate your system, which would be the equivalent of `nixos-rebuild switch`.

```sh
wire apply switch -v
```

### A Remote Machine

Lets add another node to your hive! This one is an example of a remote machine.

```nix:line-numbers [hive.nix]
{
  meta.nixpkgs = import <nixpkgs> {};

  my-local-machine = {
    imports = [./local-machine/configuration.nix];
    nixpkgs.hostPlatform = "x86_64-linux";
  };

  my-remote-machine = {
    deployment = {
      # buildOnTarget defaults to `false`, enable this
      # if the machine is strong enough to build itself.
      buildOnTarget = true;
      target = {
        # Some IP or host that this node is reachable by ssh under,
        # defaults to "my-remote-machine" (node name).
        host = "10.1.1.2";
        # A user you can non-interactively login through ssh by,
        # defaults to "root".
        user = "root";
      };
    };
    imports = [./remote-machine/configuration.nix];
    nixpkgs.hostPlatform = "x86_64-linux";
  };
}
```

> [!TIP]
> Read more options in [the reference](/reference/module#deployment-target) to adjust options such as
> ssh port.

To deploy the node `my-remote-machine`, lets use `wire apply` again. Wire will
apply both nodes in the hive at once, one local and one remote:

```sh
wire apply switch -v
```
