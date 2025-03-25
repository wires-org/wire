let
  keyPair = public: private: {
    public = builtins.toFile "public-key" public;
    private = builtins.toFile "private-key" private;
  };
in
{
  # passphraseless keys
  keys = {
    deployer = keyPair "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHLr1YpijXs5zxqskIWGSdZTic0X0vmqLuvX1xhnz3P4 deployer" ''
      -----BEGIN OPENSSH PRIVATE KEY-----
      b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
      QyNTUxOQAAACBy69WKYo17Oc8arJCFhknWU4nNF9L5qi7r19cYZ89z+AAAAJBOapfMTmqX
      zAAAAAtzc2gtZWQyNTUxOQAAACBy69WKYo17Oc8arJCFhknWU4nNF9L5qi7r19cYZ89z+A
      AAAECHqr3foydbofZ8Pqcswg5sW4PhCFRONPPD3Muv+qpc93Lr1YpijXs5zxqskIWGSdZT
      ic0X0vmqLuvX1xhnz3P4AAAACGRlcGxveWVyAQIDBAU=
      -----END OPENSSH PRIVATE KEY-----
    '';
    receiver-1 = keyPair "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDmLtXrMSi5cyH6HrQCiKFjgVN8gcHCB5Ygw0CrhHd0A receiver-1" ''
      -----BEGIN OPENSSH PRIVATE KEY-----
      b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
      QyNTUxOQAAACA5i7V6zEouXMh+h60AoihY4FTfIHBwgeWIMNAq4R3dAAAAAJCb6zQxm+s0
      MQAAAAtzc2gtZWQyNTUxOQAAACA5i7V6zEouXMh+h60AoihY4FTfIHBwgeWIMNAq4R3dAA
      AAAEAa2OYGSUbfX+kyiQVtZGtaRNA2HwC5EDayBCtg+tBXNDmLtXrMSi5cyH6HrQCiKFjg
      VN8gcHCB5Ygw0CrhHd0AAAAACnJlY2VpdmVyLTEBAgM=
      -----END OPENSSH PRIVATE KEY-----
    '';
    receiver-2 = keyPair "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILWB2/MgU8rYxmtXYsCcNpFO7DsbT6RNR7tsBxoeaXxM receiver-2" ''
      -----BEGIN OPENSSH PRIVATE KEY-----
      b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
      QyNTUxOQAAACC1gdvzIFPK2MZrV2LAnDaRTuw7G0+kTUe7bAcaHml8TAAAAJDlxy4n5ccu
      JwAAAAtzc2gtZWQyNTUxOQAAACC1gdvzIFPK2MZrV2LAnDaRTuw7G0+kTUe7bAcaHml8TA
      AAAECe9dHCZR/uaZlYVYYbnH3XlL1E+L+wIn6TnYz8FhceeLWB2/MgU8rYxmtXYsCcNpFO
      7DsbT6RNR7tsBxoeaXxMAAAACnJlY2VpdmVyLTIBAgM=
      -----END OPENSSH PRIVATE KEY-----
    '';
  };
}
