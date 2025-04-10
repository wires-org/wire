---
comment: true
title: Parallelism
description: A deeper dive into parallelism with Wire Tool.
---

# {{ $frontmatter.title }}

{{ $frontmatter.description }}

## Controlling GPU Usage

Wire evaluates, builds, pushes, and deploys each node completely independently
from each other. Internally Wire calls this process a "node execution".

The default number of parallel _node executions_ is `10`, which can be
controlled with the `-p` / `--parallel` argument.

```sh
wire apply -p <NUMBER>
```

## Interaction with Nix's `max-jobs`

Nix has an overall derivation build limit and core limit.
If executing a node fills Nix's `max-jobs` all other nodes will bottleneck. You
should read [the relevant
documentation](https://nix.dev/manual/nix/2.28/advanced-topics/cores-vs-jobs) to fine tune these settings.

When a Node is built remotely due to
[`deployment.buildOnTarget`](/reference/module.html#deployment-buildontarget)
that node will not push up the _local machine's_ max-jobs limit.
