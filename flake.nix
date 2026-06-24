# This flake file is community maintained
{
  description = "Jay: A Wayland compositor.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    # Jay requires the latest stable version of Rust.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      inherit (nixpkgs) lib;
      systems = lib.intersectLists lib.systems.flakeExposed lib.platforms.linux;
      forAllSystems =
        f:
        lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ (import rust-overlay) ];
            }
          )
        );

      jayPackage =
        {
          lib,
          stdenv,
          rustPlatform,
          autoPatchelfHook,
          installShellFiles,
          pkgconf,
          fontconfig,
          libgbm,
          libinput,
          pango,
          udev,
          xkeyboard-config,
          libglvnd,
          sqlite,
          vulkan-loader,
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
              ./jay-proc
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
            fontconfig
            libgbm
            libinput
            pango
            udev
            xkeyboard-config
          ];

          runtimeDependencies = [
            libglvnd
            sqlite.out
            vulkan-loader
          ];

          # Jay uses https://docs.rs/dlopen-note/latest/dlopen_note/ to declare its optional runtime
          # dependencies in ELF metadata (https://uapi-group.org/specifications/specs/elf_dlopen_metadata/).
          # However, auto-patchelf fails if these dependencies are not present at compile time.
          autoPatchelfIgnoreMissingDeps = [
            "libGLESv2.so.2"
            "libEGL.so.1"
            "libsqlite3.so.0"
            "libvulkan.so.1"
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

    in
    {
      devShells = forAllSystems (pkgs: {
        default =
          let
            inherit (self.packages.${pkgs.system}) jay;
            rust = pkgs.rust-bin.stable.latest.default.override {
              extensions = [
                "rust-src"
                "clippy"
                "rustfmt"
              ];
            };
          in
          pkgs.mkShell {
            inputsFrom = [ jay ];
            packages = [ rust ];
          };
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt);

      packages = forAllSystems (
        pkgs:
        let
          rust = pkgs.rust-bin.stable.latest.default;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rust;
            rustc = rust;
          };
          jay = pkgs.callPackage jayPackage {
            inherit rustPlatform;
          };
        in
        {
          inherit jay;
          default = jay;
        }
      );

      overlays.default = final: _: { inherit (self.packages.${final.system}) jay; };
    };
}
