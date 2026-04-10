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
          stdenv,
          rustPlatform,
          fetchFromGitHub,
          libGL,
          libinput,
          pkgconf,
          xkeyboard_config,
          libgbm,
          pango,
          fontconfig,
          udev,
          libglvnd,
          vulkan-loader,
          autoPatchelfHook,
          installShellFiles,
        }:

        rustPlatform.buildRustPackage {
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
              ./xml-to-wire
            ];
          };

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [
            autoPatchelfHook
            installShellFiles
            pkgconf
          ];

          buildInputs = [
            libGL
            xkeyboard_config
            libgbm
            pango
            fontconfig
            udev
            libinput
          ];

          runtimeDependencies = [
            libglvnd
            vulkan-loader
          ];

          checkFlags = [
            # the following tests require access to io_uring, which is disabled in the sandboxed build environment
            "--skip=cpu_worker::tests::cancel"
            "--skip=cpu_worker::tests::complete"
            "--skip=eventfd_cache::tests::test"
            "--skip=io_uring::ops::read_write_no_cancel::tests::cancel_in_kernel"
            "--skip=io_uring::ops::read_write_no_cancel::tests::cancel_in_userspace"
          ];

          postInstall = ''
            install -D etc/jay.portal $out/share/xdg-desktop-portal/portals/jay.portal
            install -D etc/jay-portals.conf $out/share/xdg-desktop-portal/jay-portals.conf
            install -D etc/jay.desktop $out/share/wayland-sessions/jay.desktop
          ''
          + lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
            installShellCompletion --cmd jay \
              --bash <("$out/bin/jay" generate-completion bash) \
              --zsh <("$out/bin/jay" generate-completion zsh) \
              --fish <("$out/bin/jay" generate-completion fish)
          '';

          passthru = {
            providedSessions = [ "jay" ];
          };

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
