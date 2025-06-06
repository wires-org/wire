# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - yyyy-mm-dd

### Changed

- Dependency Updates.
- Wire now compiles and includes key agents for multiple architectures, currently only linux.
- There is a new package output, `wire-small`, for testing purposes.
  It only compiles the key agent for the host that builds `wire-small`.
- `--no-progress` now defaults to true if stdin does not refer to a tty (unix pipelines, in CI).

## [0.2.0] - 2025-04-21

### Added

- Getting Started Guide by @mrshmllow.
- Web documentation for various features by @mrshmllow.
- Initial NixOS VM Testing Framework by @itslychee in https://github.com/wires-org/wire/pull/93.

### Changed

- `runtime/evaluate.nix`: force system to be null by @itslychee in https://github.com/wires-org/wire/pull/84.

> [!IMPORTANT]  
> You will have to update your nodes to include `nixpkgs.hostPlatform = "<ARCH>";`

- GH Workflows, Formatting, and other DevOps yak shaving.
- Issue Templates.
- Cargo Dependency Updates.
- `doc/` Dependency Updates.
- `flake.nix` Input Updates.

### Fixed

- Keys with a path source will now be correctly parsed as `path` instead
  of `string` by @mrshmllow in https://github.com/wires-org/wire/pull/131.
- `deployment.keys.<name>.destDir` will be automatically created if it
  does not exist. Nothing about it other than existence is guaranteed. By
  @mrshmllow in https://github.com/wires-org/wire/pull/131.
