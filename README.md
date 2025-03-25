# wire

wire is a tool to deploy nixos systems. its configuration is a superset of colmena however it is not a fork.

Read the [The Book](https://wire.althaea.zone/intro), or continue reading this readme for development information.

## Tree Layout

```
wire
├── wire
│  ├── lib
│  │  └── Rust library containing business logic, consumed by `wire`
│  ├── cli
│  │  └── Rust binary, using `lib`
│  └── key_agent
│     └── Rust binary ran on a target node. recieves key file bytes and metadata w/ protobuf over SSH stdin
├── doc
│  └── an [mdBook](https://rust-lang.github.io/mdBook/)
├── runtime
│  └── Nix files used during runtime to evaluate nodes
├── integration-testing
│  └── Integration tests using nixos tests
└──tests
   └── Directories used during cargo tests
```

## Development

Please install direnv so you can run your commits against the git hooks and use the development environment.

### Testing
#### dhat profiling

```sh
$ just build-dhat
```

#### Testing

```sh
$ cargo test
$ nix flake check
```
