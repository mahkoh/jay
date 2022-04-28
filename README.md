# Jay

Jay is a wayland compositor written in rust.

![screenshot.png](static/screenshot.png)

This project is in very early development and not yet ready for serious use.
I have been using it as a daily driver for several weeks but regressions happen often and
can go unnoticed for multiple days.

For now this repository serves purely as a code backup.
Do not expect any kind of structured commit history.

## Dependencies

While Jay is written almost completely in rust, it depends on the following libraries:

* **libinput.so**: For processing input events.
* **libEGL.so**, **libGLESv2.so**: For OpenGL rendering.
* **libgbm.so**: For graphics buffer allocation.
* **libxkbcommon.so**: For keymap handling.
* **libudev.so**: For device enumeration and hotplug support.
* **libcairo.so**, **libpangocairo-1.0.so**, **libgobject-2.0.so**, **libpango-1.0.so**: For text rendering.

Furthermore, Jay depends on the following runtime services:

* **An up-to-date linux kernel**
* **XWayland**: For XWayland support.
* **Pipewire**: For screen-recording.
* **A running X server**: For the X backend. (Only required if you want to run Jay as an X client.)
* **Logind**: For the metal backend. (Only required if you want to run Jay from a TTY.)
