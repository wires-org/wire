# referenced from https://github.com/aciceri/nixfleet/blob/master/modules/hydra/jobsets.nix
# thank you!
{
  ...
}:
let
  repo = {
    owner = "wires-org";
    name = "wire";
  };

  # nixpkgs

  mkJobset =
    {
      enabled ? 1,
      hidden ? false,
      type ? 1,
      description ? "",
      checkinterval ? 60,
      schedulingshares ? 100,
      enableemail ? false,
      emailoverride ? "",
      keepnr ? 5,
      flake,
    }:
    {
      inherit
        enabled
        hidden
        type
        description
        checkinterval
        schedulingshares
        enableemail
        emailoverride
        keepnr
        flake
        ;
    };

  mkSpec =
    contents:
    let
      escape = builtins.replaceStrings [ ''"'' ] [ ''\"'' ];
      contentsJson = builtins.toJSON contents;
    in
    builtins.derivation {
      name = "spec.json";
      system = "x86_64-linux";
      preferLocalBuild = true;
      allowSubstitutes = false;
      builder = "/bin/sh";
      args = [
        (builtins.toFile "builder.sh" ''
          echo "${escape contentsJson}" > $out
        '')
      ];
    };
in
{
  jobsets = mkSpec ({
    test-hydra = mkJobset {
      description = "${repo.name}'s main branch";
      flake = "github:${repo.owner}/${repo.name}/test-hydra";
    };

    hydra-other-branch = mkJobset {
      description = "other branch";
      flake = "github:${repo.owner}/${repo.name}/hydra-other-branch";
    };
  }
  # // (mapAttrs' (n: pr: {
  #   name = "pr_${n}";
  #   value = mkJobset {
  #     description = pr.title;
  #     flake = "github:${repo.owner}/${repo.name}/${pr.head.ref}";
  #   };
  # }) pull_requests)
  );
}
