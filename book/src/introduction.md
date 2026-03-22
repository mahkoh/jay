# Introduction

> [!NOTE]
> This book was written entirely by Claude Opus 4.6 and will likely also be
> maintained in the future with the help of AI to ensure a consistent tone of
> voice, style, and quality.
> 
> All contents have been reviewed by humans but there might still be some
> mistakes left. That would put it in good company since the artisanal,
> hand-crafted spec.yaml, that contains the TOML config specification, contains
> plenty of mistakes as well.
> 
> The writing is of high quality in my estimation, certainly better than what I
> would have produced in any reasonable amount of time. There are few obvious
> _slop_ indicators. The AI sometimes uses superlatives ("ideal", "best") that I
> would not use myself, but I've seen this style in other pieces of
> documentation, so maybe it is just a style that aims to be approachable or to
> guide users towards good solutions.
> 
> If you find something objectionable in this book, do not hesitate to open an
> issue.

Jay is a Wayland compositor for Linux with an i3-inspired tiling layout. It
supports Vulkan and OpenGL rendering, multi-GPU setups, fractional scaling,
variable refresh rate (VRR), tearing presentation, HDR, and screen sharing via
xdg-desktop-portal. X11 applications are supported through Xwayland.

Jay is configured through a declarative TOML file, with an optional advanced
mode that uses a shared library for programmatic control. A built-in
[control center](control-center.md) (opened with `alt-c`) provides a full GUI
for inspecting and changing compositor settings at runtime -- including output
arrangement, input devices, color management, GPU selection, and more. A
comprehensive command-line interface makes scripting and automation
straightforward.

See the [Features](features.md) chapter for a comprehensive overview of what
Jay can do, or jump straight to [Installation](installation.md) to get started.

## License

Jay is free software licensed under the
[GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html).

## Community

[Discord server (unofficial)](https://discord.gg/Hby736z28G)
