# wire

wire is a tool to deploy nixos systems. its configuration is a superset of colmena however it is not a fork.

Read the [The Book](https://wire.althaea.zone/intro), or continue reading this readme for development information.

## Tree Layout

```
wire
├── lib
│  └── Rust library containing business logic, consumed by `wire`
├── wire
│  └── Rust binary, using `lib`
├── key-agent
│  └── Rust binary ran on a target node. recieves key file bytes and metadata w/ protobuf over SSH stdin
├── doc
│  └── an [mdBook](https://rust-lang.github.io/mdBook/)
├── runtime
│  └── Nix files used during runtime to evaluate nodes
├── tests
│  └── Directories during cargo tests
└── local-testing
   └── To be removed
```

## Local testing

### Starting development virtual machines

```sh
$ just up
```

### dhat profiling

```sh
$ just built-dhat
```

### testing

```sh
$ cargo test
```
