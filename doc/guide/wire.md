---
comment: true
---

# What is Wire?

<p style="display: flex; gap: 8px">
  <a href="https://github.com/wires-org/wire/actions/workflows/test.yml?query=branch%3Amain">
    <img alt="Test Suite Status" src="https://img.shields.io/github/actions/workflow/status/wires-org/wire/test.yml?branch=main&style=flat-square&label=Test%20Suite">
  </a>

  <a href="https://github.com/wires-org/wire/actions/workflows/build.yml?query=branch%3Amain">
    <img alt="Build Status" src="https://img.shields.io/github/actions/workflow/status/wires-org/wire/build.yml?branch=main&style=flat-square&label=Builds">
  </a>

  <a href="https://github.com/wires-org/wire/actions/workflows/pages.yml?query=branch%3Amain">
    <img alt="Documentation Status" src="https://img.shields.io/github/actions/workflow/status/wires-org/wire/pages.yml?branch=main&style=flat-square&label=Documentation">
  </a>
</p>

Wire is a tool to deploy NixOS systems. Its configuration is a superset[^1] of [colmena](https://colmena.cli.rs/), however it is **not** a fork.

[^1]: Any colmena configuration will continue to work with wire, but wire has additional ergonomic changes you can take advantage of.

::: warning
Wire is alpha software, please use at your own risk. Many features listed in this documentation may not be complete / implemented.
:::

<div class="tip custom-block" style="padding-top: 8px">

Ready? Skip to the [Quickstart](./getting-started).

</div>

## Why Wire?

::: info
The following is the goal for a stable release and not fully implemented.
:::

| Features              | Wire                         | Colmena                                                                                                    |
| --------------------- | ---------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Secret Management     | :white_check_mark:           | :white_check_mark:                                                                                         |
| Parallel Evaluation   | :white_check_mark:           | [Experimental](https://colmena.cli.rs/unstable/features/parallelism.html#parallel-evaluation-experimental) |
| Node Tagging          | :white_check_mark:           | :white_check_mark:                                                                                         |
| `jq` pipeline support | :white_check_mark:           | :x:[^2]                                                                                                    |
| Magic Rollback        | :white_check_mark: (Planned) | :x:                                                                                                        |

[^2]: You need to write custom nix code to use Colmena hive metadata inside environments like CI pipelines, bash scripting, etc., which requires a knowledge of its internals.
