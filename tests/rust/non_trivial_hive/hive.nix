{
  meta = {
    nixpkgs = <nixpkgs>;
  };

  defaults =
    {
      name,
      nodes,
      lib,
      ...
    }:
    {
      deployment.target = {
        host = "name";
        user = "root";
      };

      assertions = [
        {
          assertion = (lib.lists.elemAt nodes 1) == name;
        }
      ];
    };

  node-a = {
    deployment.keys."a" = {
      name = "different-than-a";
      source = "hi";
    };
  };
}
