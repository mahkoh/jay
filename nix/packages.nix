{ self, pkgs, crane, specificRust }:
let
  buildInputs = with pkgs; [ 
    cairomm
    cmakeMinimal
    git
    libgbm
    libinput
    libudev-zero
    pangomm
    pkg-config
    python3Minimal
  ];
  cargoToml = "${self}/Cargo.toml";
  cargoTomlConfig = builtins.fromTOML (builtins.readFile cargoToml);
  craneLib = (crane.mkLib pkgs).overrideToolchain (p: specificRust);
  doCheck = false;
  nativeBuildInputs = with pkgs; [ pkg-config ];
  src = self;
  version = cargoTomlConfig.package.version;
in
rec {
  default = jay;

  jay = craneLib.buildPackage {
    inherit buildInputs cargoToml doCheck nativeBuildInputs src version;
    cargoArtifacts = craneLib.buildDepsOnly {
      inherit buildInputs src;
    };
    pname = "jay";
  };
}
