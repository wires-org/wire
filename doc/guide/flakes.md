---
comment: true
title: Flakes
description: Learn how to output a hive from a flake.
---

# {{ $frontmatter.title }}

{{ $frontmatter.description }}

## Output a hive

::: tip
If you have skipped ahead, please read the previous page to understand the
concept of a hive.
:::

You can use wire with a flake by outputting a hive with the `colmena` flake output.

::: code-group
<<< @/snippets/getting-started/flake.nix [flake.nix]
:::

```
❯ nix flake show
git+file:///some/path
└───colmena: unknown
```

::: tip
Notice that we did not need to specify `meta.nixpkgs`. Wire will
automatically default `meta.nixpkgs` to `inputs.nixpkgs.outPath` when the hive
is within a flake.
:::

## How to keep using `nixos-rebuild`

If any hive node shares a name with an attribute on `outputs.nixosConfigurations`, wire will merge them together.

It should be noted that there are a few downsides. For example, you cannot access `config.deployment` from `nixosConfigurations`. For this reason it would be best practice to limit configuration in `colmena` to simply defining keys and deployment options.

::: code-group
<<< @/snippets/getting-started/flake-merged.nix [flake.nix]
:::

This way, you can continue using `nixos-rebuild` and wire at the same time.
