{
  lib,
  nixosOptionsDoc,
  runCommand,
  wire-small,
  wire-dignostics-md,
  nix,
  nodejs,
  pnpm,
  stdenv,
  ...
}:
let
  eval = lib.evalModules {
    modules = [
      ../runtime/module.nix
      {
        options._module.args = lib.mkOption {
          internal = true;
        };
      }
    ];
    specialArgs = {
      name = "‹node name›";
      nodes = { };
    };
  };

  optionsMd =
    (nixosOptionsDoc {
      inherit (eval) options;
    }).optionsCommonMark;

  optionsDoc = runCommand "options-doc.md" { } ''
    cat ${optionsMd} > $out
    sed -i -e '/\*Declared by:\*/,+1d' $out
  '';

  pkg = builtins.fromJSON (builtins.readFile ./package.json);
in
stdenv.mkDerivation (finalAttrs: {
  inherit (pkg) version;
  pname = pkg.name;
  nativeBuildInputs = [
    wire-small
    nodejs
    pnpm.configHook
    nix
  ];
  src = ./.;
  pnpmDeps = pnpm.fetchDeps {
    inherit (finalAttrs) pname version src;
    fetcherVersion = 1;
    hash = "sha256-SbMM7PrJ5ck8HlFs71/ytL/18XBXaOXOR+HMWI7Wzak=";
  };
  patchPhase = ''
    cat ${optionsDoc} >> ./reference/module.md
    cat ${wire-dignostics-md} >> ./reference/errors.md
    wire inspect --markdown-help > ./reference/cli.md
  '';
  buildPhase = "pnpm run build > build.log 2>&1";
  installPhase = "cp .vitepress/dist -r $out";
  doCheck = true;
  checkPhase = ''
    nix-instantiate --eval --strict ./snippets > /dev/null
  '';
  DEBUG = "*";
})
