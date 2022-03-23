# Jay

Jay is a wayland compositor written in rust.

This project is in very early development and not yet ready for serious use.

For now this repository serves purely as a code backup.
Do not expect any kind of structured commit history.

## Dependencies

While Jay is written almost completely in rust, it has some native dependencies:

* **pixman-1.so**: For damage tracking.
* **input.so**: For processing input events.
* **EGL.so**, **GLESv2.so**: For OpenGL rendering.
* **gbm.so**: For graphics buffer allocation.
* **xkbcommon.so**: For keymap handling.
* **udev.so**: For device enumeration and hotplug support.
* **cairo.so**, **pangocairo-1.0.so**, **gobject-2.0.so**, **pango-1.0.so**: For text rendering.
