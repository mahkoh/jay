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
            fileset = lib.fileset.gitTracked ./.;
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

      nixosModule =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.programs.jay;
        in
        {
          options = {
            programs.jay = {
              enable = lib.mkEnableOption "Jay, a tiling wayland compositor";

              package = lib.mkPackageOption pkgs "jay" { };

              realtime-scheduling = lib.mkOption {
                type = lib.types.bool;
                default = true;
                description = ''
                  Wrap the Jay binary with CAP_SYS_NICE so it can elevate its scheduler to SCHED_RR
                  and create high-priority Vulkan queues, improving responsiveness under load.

                  For security, Jay only elevates to SCHED_RR if every config.so in the config directory
                  is "privileged" (owned by root:root, not group/world-writable). config.so packages built
                  by Nix and installed via the store satisfy this automatically.
                '';
              };

              xwayland.enable = lib.mkEnableOption "XWayland" // {
                default = true;
              };

              extraPackages = lib.mkOption {
                type = with lib.types; listOf package;
                default = with pkgs; [
                  alacritty
                  bemenu
                  mako
                  wl-tray-bridge
                ];
                defaultText = lib.literalExpression ''
                  with pkgs; [ alacritty bemenu mako wl-tray-bridge ];
                '';
                example = lib.literalExpression ''
                  with pkgs; [ brightnessctl wl-clipboard ]
                '';
                description = ''
                  Extra packages to be installed system wide.
                '';
              };
            };
          };

          config = lib.mkIf cfg.enable {
            environment.systemPackages =
              (lib.optional (!cfg.realtime-scheduling) cfg.package) ++ cfg.extraPackages;

            programs = {
              dconf.enable = lib.mkDefault true;
              xwayland.enable = lib.mkIf cfg.xwayland.enable (lib.mkDefault true);
            };

            security = {
              polkit.enable = true;
              pam.services.swaylock = { };

              wrappers = lib.mkIf cfg.realtime-scheduling {
                jay = {
                  owner = "root";
                  group = "root";
                  permissions = "a+rx";
                  source = lib.getExe cfg.package;
                  capabilities = "cap_sys_nice+p";
                };
              };
            };

            xdg.portal = {
              enable = lib.mkDefault true;
              configPackages = lib.mkDefault [ cfg.package ];
              extraPortals = [ pkgs.xdg-desktop-portal-gtk ];
            };

            services.displayManager.sessionPackages = [ cfg.package ];
          };
        };

      homeManagerModule =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          inherit (lib)
            literalExpression
            mkIf
            mkOption
            types
            ;

          cfg = config.wayland.windowManager.jay;
          tomlFormat = pkgs.formats.toml { };
        in
        {
          options.wayland.windowManager.jay = {
            enable = lib.mkEnableOption "Jay, a tiling wayland compositor";

            package = lib.mkPackageOption pkgs "jay" { };

            # This option is currently not used but the home-manager module tests for way-displays
            # expect this to be present for all entries of wayland.windowManager.
            systemd = {
              enable = lib.mkEnableOption null // {
                default = false;
                description = "";
              };

              variables = mkOption {
                type = types.listOf types.str;
                default = [ ];
                example = [ "--all" ];
                description = "";
              };

              extraCommands = mkOption {
                type = types.listOf types.str;
                default = [ ];
                description = "";
              };
            };

            library = mkOption {
              type = types.nullOr types.package;
              default = null;
              description = ''
                For users who need programmatic configuration beyond what TOML offers, Jay also supports
                configuration via a compiled Rust shared library using the jay-config crate. This is an
                advanced option -- the TOML config in `settings` is sufficient for the vast majority of
                use cases.

                This option expects a package that builds such a shared library (a crate with
                `crate-type = ["cdylib"]` and jay-config as a dependency) and places it at
                `$out/lib/config.so`. It is installed at ~/.config/jay/config.so.

                Jay loads config.so in preference to config.toml, so when this is set, `settings` is
                ignored by jay even though this module still writes it to disk if non-empty.
              '';
            };

            settings = mkOption {
              type = tomlFormat.type;
              default = { };
              example = literalExpression ''
                {
                  # The keymap that is used for shortcuts and also sent to clients.
                  keymap = \'\'
                    xkb_keymap {
                        xkb_keycodes { include "evdev+aliases(qwerty)" };
                        xkb_types    { include "complete"              };
                        xkb_compat   { include "complete"              };
                        xkb_symbols  { include "pc+us+inet(evdev)"     };
                    };
                  \'\';

                  # An action that will be executed when the GPU has been initialized.
                  on-graphics-initialized = [
                    { type = "exec"; exec = "mako"; }
                    { type = "exec"; exec = "wl-tray-bridge"; }
                  ];

                  # Shortcuts that are processed by the compositor.
                  shortcuts = {
                    # Focus actions
                    "alt-h" = "focus-left";
                    "alt-j" = "focus-down";
                    "alt-k" = "focus-up";
                    "alt-l" = "focus-right";

                    # Move actions
                    "alt-shift-h" = "move-left";
                    "alt-shift-j" = "move-down";
                    "alt-shift-k" = "move-up";
                    "alt-shift-l" = "move-right";

                    # Split actions
                    "alt-d" = "split-horizontal";
                    "alt-v" = "split-vertical";

                    # Toggle actions
                    "alt-t" = "toggle-split";
                    "alt-m" = "toggle-mono";
                    "alt-u" = "toggle-fullscreen";

                    # Parent/focus/close/floating
                    "alt-f" = "focus-parent";
                    "alt-c" = "open-control-center";
                    "alt-shift-c" = "close";
                    "alt-shift-f" = "toggle-floating";

                    # Exec actions
                    "Super_L" = { type = "exec"; exec = "alacritty"; };
                    "alt-p"    = { type = "exec"; exec = "bemenu-run"; };

                    # Quit and reload
                    "alt-q"        = "quit";
                    "alt-shift-r"  = "reload-config-toml";

                    # Switch to VT
                    "ctrl-alt-F1"  = { type = "switch-to-vt"; num = 1; };
                    "ctrl-alt-F2"  = { type = "switch-to-vt"; num = 2; };
                    "ctrl-alt-F3"  = { type = "switch-to-vt"; num = 3; };
                    "ctrl-alt-F4"  = { type = "switch-to-vt"; num = 4; };
                    "ctrl-alt-F5"  = { type = "switch-to-vt"; num = 5; };
                    "ctrl-alt-F6"  = { type = "switch-to-vt"; num = 6; };
                    "ctrl-alt-F7"  = { type = "switch-to-vt"; num = 7; };
                    "ctrl-alt-F8"  = { type = "switch-to-vt"; num = 8; };
                    "ctrl-alt-F9"  = { type = "switch-to-vt"; num = 9; };
                    "ctrl-alt-F10" = { type = "switch-to-vt"; num = 10; };
                    "ctrl-alt-F11" = { type = "switch-to-vt"; num = 11; };
                    "ctrl-alt-F12" = { type = "switch-to-vt"; num = 12; };

                    # Show workspace
                    "alt-F1"  = { type = "show-workspace"; name = "1"; };
                    "alt-F2"  = { type = "show-workspace"; name = "2"; };
                    "alt-F3"  = { type = "show-workspace"; name = "3"; };
                    "alt-F4"  = { type = "show-workspace"; name = "4"; };
                    "alt-F5"  = { type = "show-workspace"; name = "5"; };
                    "alt-F6"  = { type = "show-workspace"; name = "6"; };
                    "alt-F7"  = { type = "show-workspace"; name = "7"; };
                    "alt-F8"  = { type = "show-workspace"; name = "8"; };
                    "alt-F9"  = { type = "show-workspace"; name = "9"; };
                    "alt-F10" = { type = "show-workspace"; name = "10"; };
                    "alt-F11" = { type = "show-workspace"; name = "11"; };
                    "alt-F12" = { type = "show-workspace"; name = "12"; };

                    # Move to workspace
                    "alt-shift-F1"  = { type = "move-to-workspace"; name = "1"; };
                    "alt-shift-F2"  = { type = "move-to-workspace"; name = "2"; };
                    "alt-shift-F3"  = { type = "move-to-workspace"; name = "3"; };
                    "alt-shift-F4"  = { type = "move-to-workspace"; name = "4"; };
                    "alt-shift-F5"  = { type = "move-to-workspace"; name = "5"; };
                    "alt-shift-F6"  = { type = "move-to-workspace"; name = "6"; };
                    "alt-shift-F7"  = { type = "move-to-workspace"; name = "7"; };
                    "alt-shift-F8"  = { type = "move-to-workspace"; name = "8"; };
                    "alt-shift-F9"  = { type = "move-to-workspace"; name = "9"; };
                    "alt-shift-F10" = { type = "move-to-workspace"; name = "10"; };
                    "alt-shift-F11" = { type = "move-to-workspace"; name = "11"; };
                    "alt-shift-F12" = { type = "move-to-workspace"; name = "12"; };
                  };
                }
              '';
            };
          };

          config = mkIf cfg.enable {
            home.packages = [ cfg.package ];

            xdg.configFile = {
              "jay/config.toml" = mkIf (cfg.settings != { }) {
                source = tomlFormat.generate "config.toml" cfg.settings;
              };

              "jay/config.so" = mkIf (cfg.library != null) {
                source = "${cfg.library}/lib/config.so";
              };
            };
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

      nixosModules.default = nixosModule;
      homeManagerModules.default = homeManagerModule;
    };
}
