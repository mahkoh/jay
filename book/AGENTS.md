# Jay Book -- Agent Instructions

This file provides context for AI agents working on the Jay user documentation
book (mdbook). Read this before making changes.

## What is this?

An [mdbook](https://rust-lang.github.io/mdBook/) providing comprehensive
user-facing documentation for the [Jay Wayland compositor](https://github.com/mahkoh/jay).
The target audience is end users, not developers. The goal is feature
discoverability: every capability Jay offers should be documented here in a
way that users can find and understand.

## Current status

The book is feature-complete. All 29 chapters cover every known Jay feature:
a features overview, installation (including AUR packages), running, the
control center, all configuration sections, tiling, workspaces, floating
windows, mouse interactions, input modes, window/client rules, screen sharing,
HDR & color management, the full CLI reference, and troubleshooting.

Three rounds of source-code verification have been completed and all
identified issues have been fixed. The book should be accurate against the
codebase as of the latest review.

A subsequent style and formatting pass has been completed:

- Converted ~45 two-column tables to mdbook definition lists across 16 files.
- Lowercased all keyboard shortcut modifiers to match TOML parser expectations.
- Reformatted long inline TOML tables into multiline format with trailing
  commas and 4-space indentation.
- Created a dedicated Features chapter (`features.md`) consolidating content
  from the old `docs/features.md` and the introduction; removed the duplicated
  feature list from `introduction.md`.
- Added a comprehensive "Granting Privileges" section to `window-rules.md` and
  privilege troubleshooting to `troubleshooting.md`.
- Clarified `jay unlock` usage in `configuration/idle.md` and `cli.md`.
- Added AUR package (`jay`, `jay-git`) mentions to `installation.md`.
- Added a dedicated HDR & Color Management chapter (`hdr.md`) with
  end-to-end walkthrough, cross-referenced from `features.md`, `outputs.md`,
  and `misc.md`.
- Integrated remaining information from `docs/` and `README.md` that was
  missing from the book: `JAY_NO_REALTIME` env var and `config.so`/SCHED_RR
  interaction in `installation.md`, rule loop client-kill consequences and
  Xwayland override-redirect auto-focus exception in `window-rules.md`,
  license (GPLv3) and community Discord link in `introduction.md`, ArchWiki
  XKB link in `keymaps.md`.

A third review pass fixed:

- `window-rules.md`: exec action example used a plain string with spaces
  (`exec = "notify-send 'Firefox connected'"`) which would be treated as a
  single program name; changed to array form
  (`exec = ["notify-send", "Firefox connected"]`).
- `window-rules.md`: default capabilities paragraph incorrectly said rule
  capabilities are "added on top" of defaults; corrected to say they
  **replace** defaults entirely.
- `troubleshooting.md`: said Jay "requires at least one working renderer to
  start"; corrected to say that without one, no GPU can be initialized and
  nothing will be displayed (Jay itself still starts).
- `outputs.md`: VRR variant3 description said "describes its content as"
  instead of the correct protocol term "describes its content type as".

A fourth update documented the new `--json` and `--all-json-fields` global
CLI flags, which enable machine-readable JSONL output from all query/status
subcommands. A new "JSON Output" section was added to `cli.md` listing all
supported commands, the JSONL format, field omission behavior, and `jq`
usage examples.

**Future work might include:**

- Keeping the book in sync as Jay adds new features or changes behavior.
- Adding screenshots or diagrams (especially for the control center and tiling).
- Reviewing after spec.yaml changes (the spec is the canonical source for
  config options).

## Directory layout

```
book/
  AGENTS.md          -- this file
  book.toml          -- mdbook configuration
  src/
    SUMMARY.md       -- table of contents (defines chapter ordering)
    introduction.md
    features.md      -- feature overview, protocol table, CLI help output
    installation.md  -- deps, AUR, crates.io, git builds, CAP_SYS_NICE
    running.md       -- includes control center intro and default keybindings
    control-center.md -- detailed tour of all 11 panes
    configuration/
      index.md       -- config overview, jay config init, file semantics, auto-reload
      keymaps.md
      shortcuts.md
      outputs.md
      inputs.md
      gpu.md
      idle.md
      theme.md
      environment.md
      status-bar.md
      startup.md
      xwayland.md
      misc.md        -- color management, libei, floating, ui-drag, etc.
    tiling.md
    workspaces.md
    floating.md
    mouse.md
    input-modes.md
    window-rules.md  -- includes "Granting Privileges" section
    cli.md
    screen-sharing.md
    hdr.md           -- HDR & color management walkthrough
    troubleshooting.md -- includes privilege troubleshooting
```

## Source of truth

The authoritative source for what Jay supports is the source code itself. Key
reference files:

- `toml-spec/spec/spec.yaml` -- Full TOML config specification. Every config
  key, action, match criterion, and type is defined here. The auto-generated
  `toml-spec/spec/spec.generated.md` is derived from this file.
- `toml-config/src/default-config.toml` -- The built-in default configuration.
  Contains default keybindings, startup actions (mako, wl-tray-bridge), etc.
- `src/cli/*.rs` -- CLI subcommands and their arguments (clap definitions).
  Jay has 24 CLI subcommands with nesting up to 4 levels deep.
- `src/control_center/*.rs` -- Control center pane implementations. Each pane
  has its own file (cc_outputs.rs, cc_gpus.rs, cc_input.rs, cc_clients.rs,
  cc_window.rs, cc_idle.rs, cc_compositor.rs, cc_look_and_feel.rs,
  cc_xwayland.rs, cc_color_management.rs, cc_virtual_outputs.rs). When
  documenting control center fields, always verify against these source files
  for correct ordering, labels, and conditional visibility.
- `toml-config/src/config/parsers/color.rs` -- Color parser. Accepts 3, 4, 6,
  or 8 hex digits (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`). Note: spec.yaml
  says `#rrggbba` (7 digits) which is a typo -- the actual format is 8 digits.
- `toml-config/src/config/parsers/exec.rs` -- Exec parser. Accepts string,
  array, or table. Understanding these three forms is important for writing
  correct TOML examples.
- `src/config/handler.rs` -- Contains `update_capabilities`, which shows that
  client rule capabilities **replace** defaults (accumulator starts from zero).
- The `docs/` directory has been removed. All of its content has been
  integrated into the book. The README now points to the hosted book at
  `https://mahkoh.github.io/jay/book`.

### Known spec.yaml bugs

- `px-per-wheel-scroll` is listed as `kind: boolean` but the parser
  (`toml-config/src/config/parsers/input.rs`) uses `fltorint` (a number).
  The documentation correctly describes it as a numeric value.

## Critical facts to get right

1. **Config replacement semantics.** Once `~/.config/jay/config.toml` exists,
   the entire built-in default configuration is replaced -- not merged. Even an
   empty config file means no shortcuts, no startup actions, nothing. Users must
   run `jay config init` to get a config pre-populated with the defaults.

2. **Config reload.** By default, Jay does not automatically reload
   config.toml. Reload must be triggered manually via the `alt-shift-r`
   shortcut (default) or the `reload-config-toml` action. However, setting
   `auto-reload = true` in config.toml enables inotify-based file watching
   with a 400 ms debounce (parsed in
   `toml-config/src/config/parsers/config.rs:209`, acted on in
   `toml-config/src/lib.rs:1348-1358`).

3. **Renderer requirement.** Jay requires at least one working renderer
   (OpenGL via libEGL+libGLESv2, or Vulkan via libvulkan + a GPU driver).
   Without one, no GPU can be initialized and nothing will be displayed. (Jay
   itself still starts -- the metal backend initializes logind, udev, and
   libinput successfully -- but no DRM devices can be set up, so the user sees
   a black screen.) These libraries are loaded at runtime, not linked at build
   time. Vulkan is the recommended renderer; OpenGL is legacy and receives no
   new features.

4. **Actions are composable.** Anywhere an action is accepted, an array of
   actions can be used instead. Named actions (`$name`) add another layer of
   reuse.

5. **Match systems.** Outputs, connectors, DRM devices, inputs, clients, and
   windows each have their own match system. All support the same logical
   combinators: AND (multiple fields in one table), OR (array of matchers),
   `not`, `all`, `any`, `exactly`.

6. **Input modes.** Jay supports an input mode stack (`push-mode`,
   `latch-mode`, `pop-mode`, `clear-modes`). Each mode can define its own
   shortcuts and inherit from a parent mode. This is how vim-style modal
   keybindings are implemented. Note: there are NO `resize-left/right/up/down`
   actions -- resizing is mouse-only. Use `move-*` actions in examples.

7. **Window/client rules are reactive.** Rules are re-evaluated whenever the
   matching criteria change (e.g. a window's title changes). The `latch` action
   fires when a previously-matching rule stops matching.

8. **Control center.** Opened with `alt-c` (default) or `jay control-center`.
   It is an egui-based GUI with 11 panes: Clients, Color Management,
   Compositor, GPUs, Idle, Input, Look and Feel, Outputs (with visual
   arrangement editor), Virtual Outputs, Window Search, and Xwayland.

9. **VRR and tearing.** Both have per-output overrides and multiple mode
   variants (never, always, variant1/2/3). Important distinction: VRR
   variant1/2 use "fullscreen" as the criterion. Tearing variant3 says "a
   single application is displayed" (not necessarily fullscreen) -- check the
   spec.yaml wording carefully.

10. **Input device type flags.** The full list is: `is-keyboard`, `is-pointer`,
    `is-touch`, `is-tablet-tool`, `is-tablet-pad`, `is-gesture`, `is-switch`.
    Note that `is-switch` exists in the parser but is missing from spec.yaml.

11. **Features chapter is the single source for the feature list.** The
    introduction links to `features.md` but does not duplicate any feature
    bullets. All feature descriptions, the protocol support table, and CLI help
    output live in `features.md` only.

12. **Privilege separation.** Jay supports granting elevated Wayland protocol
    access via three methods: `privileged = true` in window rules with
    `jay run-privileged`, connection tags with client rules, and client match
    rules by PID/sandboxing. This is documented in the "Granting Privileges"
    section of `window-rules.md`. **Important:** when any client rule matches,
    its capabilities **replace** the defaults entirely (they are not added on
    top). Multiple matching rules are unioned together, but the defaults
    (`layer-shell | drm-lease` for unsandboxed, `drm-lease` for sandboxed)
    are only used when no rule matches at all.

13. **Exec action formats.** The `exec` field in exec actions accepts three
    forms: a plain string (program name only, no arguments), an array of
    strings (first element is the program, rest are arguments), or a table
    with `prog`/`shell`, `args`, `env`, `privileged`, and `tag` fields. A
    common mistake is using a plain string with spaces like
    `exec = "notify-send 'hello'"` -- this treats the entire string as the
    program name. Use the array form instead:
    `exec = ["notify-send", "hello"]`.

## Style guidelines

- Write for users, not developers. Explain what things do, not how they are
  implemented.
- Use ` ```shell ` for shell commands, prefixed with `~$` and `sudo` where
  appropriate.
- Use ` ```toml ` for TOML configuration examples.
- Use **mdbook admonitions** (`> [!NOTE]`, `> [!TIP]`, `> [!WARNING]`,
  `> [!IMPORTANT]`) instead of `> **Note:**` blockquote patterns. mdbook
  0.5.2+ is installed and supports this syntax.
- **Keyboard shortcuts** must be enclosed in backticks and use **lowercase
  modifiers** -- this matches the format the TOML parser expects. Keysym names
  retain their original case. Examples: `alt-shift-c`, `ctrl-alt-F2`,
  `alt-Return`. Never write `Alt-Shift-c` or `Alt+C`.
- **Tables vs definition lists.** Use mdbook definition lists (`Term\n:
  Description`) for two-column term-to-description mappings. Keep true
  grid-like tables only when they have 3+ meaningful data columns (e.g.,
  package names per distro, the protocol support table with version numbers).
- **TOML formatting.** Break long inline TOML tables over multiple lines with
  trailing commas (TOML 1.1 syntax). Use 4-space indentation inside TOML code
  blocks, never 2-space.
- Keep examples practical -- show real use cases, not abstract syntax.
- Link to the auto-generated spec (`spec.generated.md`) for exhaustive field
  listings. The book should teach concepts and workflows; the spec is the
  complete reference.
- Each chapter should be self-contained enough to be useful on its own, but
  cross-reference related chapters where helpful.
- Avoid duplicating large tables of every possible value. Summarize, give
  examples, and link to the spec for the full list.
- When documenting the control center, always verify field names, ordering,
  and conditional visibility against the `src/control_center/cc_*.rs` source
  files. UI labels must match exactly (case-sensitive).

## Verification methodology

When making changes, verify against source code. The most error-prone areas
are:

1. **Control center pane documentation** -- Field names, ordering, and
   conditional visibility must match the `show_*` functions in the cc_*.rs
   files. Fields are often conditional (only shown for certain device types or
   when certain values are set).

2. **VRR/tearing mode descriptions** -- The exact semantics differ subtly
   between modes and between VRR vs tearing. Always check spec.yaml wording.

3. **Exec action examples** -- A plain string is the program name only (no
   argument splitting). Use arrays or tables when arguments are needed.

4. **Capability/privilege semantics** -- Capabilities from matching rules
   replace defaults; they do not add to them. Verify against
   `src/config/handler.rs:update_capabilities` if in doubt.

## Building the book

```shell
~$ cd book
~$ mdbook build    # outputs to book/book/
~$ mdbook serve    # local preview at http://localhost:3000
```

Requires [mdbook](https://github.com/rust-lang/mdBook) to be installed:

```shell
~$ cargo install mdbook
```
