let
  inherit (import ../../..) makeHive;
in
makeHive {
  node-a = { };
}
