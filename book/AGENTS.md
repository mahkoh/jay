# Jay Book -- Agent Instructions

User-facing [mdbook](https://rust-lang.github.io/mdBook/) documentation for
the [Jay Wayland compositor](https://github.com/mahkoh/jay). Target audience
is end users, not developers. Goal is feature discoverability.

## Quick reference

### Book files (`book/src/`)

The table of contents is `SUMMARY.md`. Key chapter-to-topic mapping:

| File | Covers |
|------|--------|
| `configuration/shortcuts.md` | Shortcuts, actions (simple + parameterized), marks, named actions, virtual outputs, actions in window rules |
| `configuration/outputs.md` | Monitor config, VRR, tearing, scaling, transforms |
| `configuration/inputs.md` | Input devices, per-device settings |
| `configuration/misc.md` | Color management, libei, floating defaults, ui-drag |
| `window-rules.md` | Window/client rules, privileges, capabilities |
| `control-center.md` | All 11 control center panes |
| `cli.md` | All CLI subcommands, JSON output |
| `input-modes.md` | Modal keybinding system (push/pop/latch/clear) |
| `floating.md` | Floating windows, window management mode |
| `hdr.md` | HDR & color management walkthrough |

### Source-of-truth files (from repo root)

| File | What it tells you |
|------|-------------------|
| `toml-spec/spec/spec.yaml` | **Canonical** TOML config spec: every key, action, match criterion, type |
| `toml-config/src/default-config.toml` | Built-in default config (keybindings, startup actions) |
| `toml-config/src/config/parsers/action.rs` | Action parser — see which `type` strings are accepted |
| `toml-config/src/lib.rs` | Action dispatch — `window_or_seat!` macro shows which actions work in window rules |
| `src/config/handler.rs` | Config handler; `update_capabilities` shows capability replacement semantics |
| `src/cli/*.rs` | CLI subcommands (clap definitions) |
| `src/control_center/cc_*.rs` | Control center pane implementations (verify field names/ordering here) |
| `toml-config/src/config/parsers/exec.rs` | Exec parser (string, array, or table forms) |

### Known spec.yaml bugs

- `px-per-wheel-scroll`: listed as `kind: boolean` but parser uses `fltorint`
  (a number). Book correctly documents it as numeric.

## Critical facts

1. **Config replacement.** `~/.config/jay/config.toml` replaces the entire
   built-in default — not merged. Empty file = no shortcuts, nothing.
   Users must run `jay config init`.

2. **Config reload.** Manual by default (`alt-shift-r` / `reload-config-toml`).
   `auto-reload = true` enables inotify watching with 400 ms debounce.

3. **Actions are composable.** Anywhere an action is accepted, an array works.
   Named actions (`$name`) add reuse.

4. **Exec action gotcha.** Plain string = program name only (no arg splitting).
   `exec = "notify-send 'hello'"` is wrong — use array form.

5. **Capability replacement.** When any client rule matches, its capabilities
   **replace** defaults entirely (not additive). Verify against
   `handler.rs:update_capabilities`.

6. **Window rule actions.** Actions using `window_or_seat!` in `lib.rs` work
   in both shortcuts (focused window) and window rules (matched window).
   The list in shortcuts.md "Actions in window rules" must stay in sync.

7. **Window/client rules are reactive.** Re-evaluated when criteria change.
   `latch` fires when a rule stops matching.

8. **VRR vs tearing subtlety.** VRR variant1/2 use "fullscreen"; tearing
   variant3 says "a single application is displayed" (not necessarily
   fullscreen). Always check spec.yaml wording.

## Style rules

- **Audience:** users, not developers. Explain what, not how.
- **Code fences:** ` ```shell ` for commands (prefix `~$`), ` ```toml ` for config.
- **Admonitions:** `> [!NOTE]`, `> [!TIP]`, `> [!WARNING]`, `> [!IMPORTANT]`.
- **Key combos:** backticks, lowercase modifiers: `alt-shift-c`, `ctrl-alt-F2`,
  `alt-Return`. Never `Alt+C`.
- **Definition lists** for two-column term/description. Tables only for 3+ data columns.
- **TOML formatting:** multiline with trailing commas, 4-space indent.
- **Examples:** practical, not abstract. Link to
  [spec.generated.md](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md)
  for exhaustive listings.
- **Control center docs:** verify field names, ordering, and conditional
  visibility against `cc_*.rs` source files. Labels must match exactly.

## Common tasks

### Documenting a new action

1. Read `git diff` for the commit introducing the action. Key files:
   - `toml-spec/spec/spec.yaml` — spec entry (description, fields, examples)
   - `toml-config/src/config/parsers/action.rs` — parser (field names, types, defaults)
   - `toml-config/src/lib.rs` — dispatch (check if `window_or_seat!` is used)
   - `jay-config/src/input.rs` and/or `jay-config/src/window.rs` — Rust API

2. Edit `book/src/configuration/shortcuts.md`:
   - **Simple actions** (no fields): add to the appropriate list in the
     "Simple actions" section.
   - **Parameterized actions** (has fields): add a new `###` subsection before
     "Other parameterized actions". Include definition list for fields and
     practical TOML examples.
   - **Also parameterized but minor:** just add a `- name -- description`
     bullet to the "Other parameterized actions" list.

3. If `window_or_seat!` is used in `lib.rs`, add the action name to the
   "Actions in window rules" list at the bottom of `shortcuts.md`.

### Documenting a new config field

1. Read `toml-spec/spec/spec.yaml` for the field definition.
2. Identify which book chapter covers that config section (see table above).
3. Add the field with a definition-list entry or example, matching the
   existing style of that chapter.

### Documenting a new CLI subcommand

1. Read `src/cli/*.rs` for clap definitions.
2. Edit `book/src/cli.md`. Follow the existing pattern for subcommand docs.
3. If the subcommand has `--json` support, mention it in the "JSON Output" section.

### Documenting a control center change

1. Read the relevant `src/control_center/cc_*.rs` file.
2. Edit `book/src/control-center.md`. Match field names, ordering, and
   conditional visibility exactly.

## Building

```shell
~$ cd book && mdbook build    # outputs to book/book/
~$ cd book && mdbook serve    # preview at http://localhost:3000
```
