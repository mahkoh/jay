# This flake file is community maintained
{
  description = "Jay: A Wayland compositor.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      jay-package =
        {
          lib,
          rustPlatform,
          fetchFromGitHub,
          libGL,
          libinput,
          pkgconf,
          xkeyboard_config,
          libgbm,
          pango,
          udev,
          shaderc,
          libglvnd,
          vulkan-loader,
          autoPatchelfHook,
        }:

        rustPlatform.buildRustPackage rec {
          pname = "jay";
          version = self.shortRev or self.dirtyShortRev or "unknown";

          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./algorithms
              ./build
              ./Cargo.lock
              ./Cargo.toml
              ./etc
              ./jay-config
              ./rustfmt.toml
              ./src
              ./toml-config
              ./toml-spec
              ./wire
              ./wire-dbus
              ./wire-ei
              ./wire-to-xml
              ./wire-xcon
            ];
          };

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          SHADERC_LIB_DIR = "${lib.getLib shaderc}/lib";

          nativeBuildInputs = [
            autoPatchelfHook
            pkgconf
          ];

          buildInputs = [
            libGL
            xkeyboard_config
            libgbm
            pango
            udev
            libinput
            shaderc
          ];

          runtimeDependencies = [
            libglvnd
            vulkan-loader
          ];

          postInstall = ''
            install -D etc/jay.portal $out/share/xdg-desktop-portal/portals/jay.portal
            install -D etc/jay-portals.conf $out/share/xdg-desktop-portal/jay-portals.conf
          '';

          meta = with lib; {
            description = "Wayland compositor written in Rust";
            homepage = "https://github.com/mahkoh/jay";
            license = licenses.gpl3;
            platforms = platforms.linux;
            mainProgram = "jay";
          };
        };

      inherit (nixpkgs) lib;
      # Support all Linux systems that the nixpkgs flake exposes
      systems = lib.intersectLists lib.systems.flakeExposed lib.platforms.linux;

      forAllSystems = lib.genAttrs systems;
      nixpkgsFor = forAllSystems (system: nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgsFor.${system};
          inherit (self.packages.${system}) jay;
        in
        {
          default = pkgs.mkShell {
            packages = [
              pkgs.rustc
              pkgs.cargo
              pkgs.clippy
              pkgs.rustfmt
            ];

            nativeBuildInputs = [
              pkgs.pkg-config
            ];

            buildInputs = jay.buildInputs;
          };
        }
      );

      formatter = forAllSystems (system: nixpkgsFor.${system}.nixfmt-rfc-style);

      packages = forAllSystems (
        system:
        let
          jay = nixpkgsFor.${system}.callPackage jay-package { };
        in
        {
          inherit jay;
          default = jay;
        }
      );

      overlays.default = final: _: {
        jay = final.callPackage jay-package { };
      };
    };
}
