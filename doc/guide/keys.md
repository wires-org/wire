---
comment: true
title: Secret Management
description: Keys, files, and other out-of-store paths with Wire Tool.
---

# {{ $frontmatter.title }}

{{ $frontmatter.description }}

::: warning

Pushing keys to your local machine is currently unimplemented and is planned for
v1.0.0.

:::

## Introduction

Wire Tool is very unopinionated as to how you encrypt your secrets, Wire only
handles pushing and setting up permissions of your key files.

The `source` of your key can be a literal string (unencrypted), a path
(unencrypted), or a command that wire runs to evaluate the key. Programs that
work well with wire keys include:

- GPG
- [Age](https://github.com/FiloSottile/age)
- Anything that non-interactively decrypts to `stdout`.

### A Trivial "Key"

```nix:line-numbers [hive.nix]
{
  meta.nixpkgs = import <nixpkgs> {};

  node-1 = {
    deployment.key."file.txt" = {
      source = ''
        Hello World!
      '';
    };
  };
}
```

```sh
[user@node-1]$ cat /run/keys/file.txt
Hello World!
```

### Encrypting with GPG

```nix:line-numbers [hive.nix]
{
  meta.nixpkgs = import <nixpkgs> {};

  node-1 = {
    deployment.key."file.txt" = {
      source = [
        "gpg"
        "--decrypt"
        "${./secrets/file.txt.gpg}"
      ];
    };
  };
}
```

```sh
[user@node-1]$ cat /run/keys/file.txt
Hello World!
```

### A Plain Text File

```nix:line-numbers [hive.nix]
{
  meta.nixpkgs = import <nixpkgs> {};

  node-1 = {
    deployment.key."file.txt" = {
      # using this syntax will enter the file into the store, readable by
      # anyone!
      source = ./file.txt;
    };
  };
}
```

## Persistence

Wire defaults `destDir` to `/run/keys`. `/run/` is held in memory and will not
persist past reboot. Change
[`deployment.key.<name>.destDir`](/reference/module#deployment-keys-name-destdir)
to something like `/etc/keys` if you need secrets every time the machine boots.

## Upload Order

By default Wire will upload keys before the system is activated. You can
force Wire to upload the key after the system is activated by setting
[`deployment.keys.<name>.uploadAt`](/reference/module#deployment-keys-name-uploadat)
to `post-activation`.

## Permissions and Ownership

Wire secrets are owned by user & group `root` (`0600`). You can change these
with the `user` and `group` option.

```nix:line-numbers [hive.nix]
{
  meta.nixpkgs = import <nixpkgs> {};

  node-1 = {
    deployment.key."file.txt" = {
      source = [
        "gpg"
        "--decrypt"
        "${./secrets/file.txt.gpg}"
      ];

      user = "my-user";
      group = "my-group";
    };
  };
}
```

## Further Examples

### Using Keys With Services

You can access the full absolute path of any key with
`config.deployment.keys.<name>.path` (auto-generated and read-only).
Here's an example with the Tailscale service:

```nix:line-numbers [hive.nix]
{
  meta.nixpkgs = import <nixpkgs> {};

  node-1 = {config, ...}: {
    services.tailscale = {
      enable = true;
      # use deployment key path directly
      authKeyFile = config.deployment.keys."tailscale.key".path;
    };

    deployment.keys."tailscale.key" = {
      keyCommand = ["gpg" "--decrypt" "${./secrets/tailscale.key.gpg}"];
    };
  };
}
```

### Scoping a Key to a service account

Additionally you can scope the key to the user that the service runs under, to
further reduce duplication using the `config` argument. Here's an example of
providing a certificate that is only readable by the caddy service.

```nix:line-numbers [hive.nix]
{
  meta.nixpkgs = import <nixpkgs> {};

  some-web-server = {config, ...}: {
    deployment.keys."some.host.pem" = {
      keyCommand = ["gpg" "--decrypt" "${./some.host.pem.gpg}"];
      destDir = "/etc/keys";

      # inherit the user and group that caddy runs under
      # the key will only readable by the caddy service
      inherit (config.services.caddy) user group;
    };

    # ^^ repeat for `some.host.key`

    services.caddy = {
      virtualHosts."https://some.host".extraConfig = ''
        tls ${config.deployment.keys."some.host.pem".path} ${config.deployment.keys."some.host.key".path}
      '';
    };
  };
}
```
