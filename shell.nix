{ pkgs ? import <nixpkgs> {
    builtins = [(import (fetchTarball {
      url    = "https://github.com/NixOS/nixpkgs/archive/e06c5e01088672bc460b2bc6b61d88e95190a492.tar.gz";
      sha256 = "sha256:e7d37547638aeb6b70a9dbf6dcc5970529edef39b46760a1c9689ac7f066ed58";
    }))];
    overlays = [
      (import (fetchGit {
        url = "https://github.com/oxalica/rust-overlay.git";
        rev = "0e624f2b1972a34be1a9b35290ed18ea4b419b6f";
      }))
    ];
   }
}:

pkgs.mkShell {
  name = "overwatch-build-shell";

  buildInputs = with pkgs; [
    pkg-config
    # Updating the version here requires also updating the `rev` version in the `overlays` section above
    # with a commit that contains the new version in its manifest
    rust-bin.stable."1.87.0".default
    go_1_19
  ];
}
