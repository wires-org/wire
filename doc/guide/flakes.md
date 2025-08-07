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

You can use wire with a flake by outputting a hive with the `wire` flake output.
Just like when using a `hive.nix`, you must provide `meta.nixpkgs` which will
come from an input.

::: code-group
<<< @/snippets/getting-started/flake.nix [flake.nix]
:::

```
❯ nix flake show
git+file:///some/path
└───colmena: unknown
```

## How to keep using `nixos-rebuild`

You can provide `makeHive` with your `nixosConfigurations` with the `inherit`
nix keyword. `makeHive` will merge any nodes and nixosConfigurations that share
the same name together.

::: tip
It should be noted that there are a few downsides. For example, you cannot access `config.deployment` from `nixosConfigurations`. For this reason it would be best practice to limit configuration in `colmena` to simply defining keys and deployment options.
:::

::: code-group
<<< @/snippets/getting-started/flake-merged.nix [flake.nix]
:::

Now, if we run `wire show`, you will see that wire only finds
the `nixosConfigurations`-es that also match a node in the hive.

```
❯ nix run ~/Projects/wire#wire-small -- show
Hive {
    nodes: {
        Name(
            "node-a",
        ): Node {
            target: Target {
                hosts: [
                    "node-a",
                ],
                user: "root",
                port: 22,
                current_host: 0,
            },
            build_remotely: false,
            allow_local_deployment: true,
            tags: {},
            keys: [],
            host_platform: "x86_64-linux",
        },
    },
    schema: 0,
}
```

This way, you can continue using `nixos-rebuild` and wire at the same time.
