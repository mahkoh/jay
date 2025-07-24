{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";

    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        cargoToml = (builtins.fromTOML (builtins.readFile ./Cargo.toml));
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit overlays system;
        };
      in
      {
        devShells = {
          default = pkgs.mkShell ({
            buildInputs = with pkgs; [ 
              cairomm
              cmakeMinimal
              git
              libgbm
              libinput
              libudev-zero
              pangomm
              pkg-config
              pkgs.rust-bin.stable.${cargoToml.package.rust-version}.minimal
              python3Minimal
            ];
          });
        };
        packages = (import ./nix/packages.nix { 
          inherit self pkgs crane;
          # The default Rust(1.86.0) in 25.05 is older than MSRV, so we use latest one.
          specificRust = pkgs.rust-bin.stable.latest.minimal;
        });
      }
    );
}
