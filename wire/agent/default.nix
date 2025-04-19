{
  perSystem =
    {
      buildRustProgram,
      ...
    }:
    {
      packages = {
        agent = buildRustProgram {
          name = "agent";
          pname = "agent";
          cargoExtraArgs = "-p agent";
        };
      };
    };
}
