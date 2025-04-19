---
comment: true
title: Targeting Nodes
description: Tags, nodes, and how to target them with Wire Tool.
---

# {{ $frontmatter.title }}

{{ $frontmatter.description }}

## The Basics

Nodes can have _tags_, which allows you to easily target multiple, related
nodes for deployment.

```nix:line-numbers [hive.nix]
{
  meta.nixpkgs = import <nixpkgs> {};

  node-1 = {
    # ...
    deployment.tags = ["cloud"];
  };
  node-2 = {
    # ...
    deployment.tags = ["cloud", "virtual"];
  };
  node-3 = {
    # ...
    deployment.tags = ["on-prem"];
  };
  node-4 = {
    # ...
    deployment.tags = ["virtual"];
  };
  node-5 = {
    # Untagged
  };
}
```

To target all nodes with a specific tag, prefix tags with an `@`.
For example, to deploy only nodes with the `cloud` tag, use

```sh
wire apply --on @cloud
```

## Further Examples

::: info

Other operations such as intersection or a theoretical `--ignore` argument
(subtracting a set of nodes) are unimplemented as of wire `v0.1.0`.

:::

### Mixing Tags with Node Names

`--on` without an `@` prefix interprets as a literal node name. You can mix tags
and node names with `--on`:

```sh
wire apply --on @cloud node-5
```

This will deploy all nodes in `@cloud`, alongside the node `node-5`.

### Targeting Many Tags (Union)

You can specify many tags together:

```sh
wire apply --on @cloud @on-prem
```

This is equivalent to a union between the set of nodes with tag `@cloud` and the
set of nodes with tag `@on-prem`.
