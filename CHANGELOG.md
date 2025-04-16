# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - yyyy-mm-dd

### Added

- v1.0.0 Getting Started Guide by @mrshmllow.
- v1.0.0 Web documentation for various features by @mrshmllow.
- v1.0.0 Initial NixOS VM Testing Framework by @itslychee in https://github.com/wires-org/wire/pull/93.
- v1.0.0 Run tests against `unstable`, `24.11` and `24.05` by @mrshmllow in https://github.com/wires-org/wire/pull/122.

### Changed

- v1.0.0 `runtime/evaluate.nix`: force system to be null by @itslychee in https://github.com/wires-org/wire/pull/84.

> [!IMPORTANT]  
> You will have to update your nodes to include `nixpkgs.hostPlatform = "<ARCH>";`

- v1.0.0 GH Workflows, Formatting, and other DevOps yak shaving.
- v1.0.0 Issue Templates.
- v1.0.0 Cargo Dependency Updates.
- v1.0.0 `doc/` Dependency Updates.
- v1.0.0 `flake.nix` Input Updates.
