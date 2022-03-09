use crate::open;
use repc::layout::{Type, TypeVariant};
use std::env;
use std::fmt::Write as FmtWrite;
use std::io::Write;

#[allow(unused_macros)]
#[macro_use]
#[path = "../src/macros.rs"]
mod macros;

#[path = "../src/pixman/consts.rs"]
mod pixman;

#[path = "../src/xkbcommon/consts.rs"]
mod xkbcommon;

#[path = "../src/libinput/consts.rs"]
mod libinput;

fn get_target() -> repc::Target {
    let rustc_target = env::var("TARGET").unwrap();
    repc::TARGET_MAP
        .iter()
        .cloned()
        .find(|t| t.0 == rustc_target)
        .unwrap()
        .1
}

fn get_enum_ty(variants: Vec<i128>) -> anyhow::Result<u64> {
    let target = get_target();
    let ty = Type {
        layout: (),
        annotations: vec![],
        variant: TypeVariant::Enum(variants),
    };
    let ty = repc::compute_layout(target, &ty)?;
    assert!(ty.layout.pointer_alignment_bits <= ty.layout.size_bits);
    Ok(ty.layout.size_bits)
}

fn write_ty<W: Write>(f: &mut W, vals: &[u32], ty: &str) -> anyhow::Result<()> {
    let variants: Vec<_> = vals.iter().cloned().map(|v| v as i128).collect();
    let size = get_enum_ty(variants)?;
    writeln!(f, "#[allow(dead_code)]")?;
    writeln!(f, "pub type {} = u{};", ty, size)?;
    Ok(())
}

fn write_egl_procs<W: Write>(f: &mut W) -> anyhow::Result<()> {
    let map = [
        (
            "eglGetPlatformDisplayEXT",
            "EGLDisplay",
            &[
                ("platform", "EGLenum"),
                ("native_display", "*mut u8"),
                ("attrib_list", "*const EGLint"),
            ][..],
        ),
        (
            "eglCreateImageKHR",
            "EGLImageKHR",
            &[
                ("dpy", "EGLDisplay"),
                ("ctx", "EGLContext"),
                ("target", "EGLenum"),
                ("buffer", "EGLClientBuffer"),
                ("attrib_list", "*const EGLint"),
            ][..],
        ),
        (
            "eglDestroyImageKHR",
            "EGLBoolean",
            &[("dpy", "EGLDisplay"), ("image", "EGLImageKHR")][..],
        ),
        (
            "eglQueryDmaBufFormatsEXT",
            "EGLBoolean",
            &[
                ("dpy", "EGLDisplay"),
                ("max_formats", "EGLint"),
                ("formats", "*mut EGLint"),
                ("num_formats", "*mut EGLint"),
            ][..],
        ),
        (
            "eglQueryDmaBufModifiersEXT",
            "EGLBoolean",
            &[
                ("dpy", "EGLDisplay"),
                ("format", "EGLint"),
                ("max_modifiers", "EGLint"),
                ("modifiers", "*mut EGLuint64KHR"),
                ("external_only", "*mut EGLBoolean"),
                ("num_modifiers", "*mut EGLint"),
            ][..],
        ),
        (
            "eglDebugMessageControlKHR",
            "EGLint",
            &[
                ("callback", "EGLDEBUGPROCKHR"),
                ("attrib_list", "*const EGLAttrib"),
            ][..],
        ),
        (
            "eglQueryDisplayAttribEXT",
            "EGLBoolean",
            &[
                ("dpy", "EGLDisplay"),
                ("attribute", "EGLint"),
                ("value", "*mut EGLAttrib"),
            ][..],
        ),
        (
            "eglQueryDeviceStringEXT",
            "*const c::c_char",
            &[("device", "EGLDeviceEXT"), ("name", "EGLint")][..],
        ),
        (
            "eglQueryDevicesEXT",
            "EGLBoolean",
            &[
                ("max_devices", "EGLint"),
                ("devices", "*mut EGLDeviceEXT"),
                ("num_devices", "*mut EGLint"),
            ][..],
        ),
        (
            "glEGLImageTargetRenderbufferStorageOES",
            "()",
            &[("target", "GLenum"), ("image", "GLeglImageOES")][..],
        ),
        (
            "glEGLImageTargetTexture2DOES",
            "()",
            &[("target", "GLenum"), ("image", "GLeglImageOES")][..],
        ),
    ];

    writeln!(f, "use std::ptr;")?;
    writeln!(f, "use super::gl::sys::*;")?;
    writeln!(f, "use super::egl::sys::*;")?;
    writeln!(f)?;
    writeln!(f, "#[derive(Copy, Clone, Debug)]")?;
    writeln!(f, "pub struct ExtProc {{")?;
    for (name, _, _) in map.iter().copied() {
        writeln!(f, "    {}: *mut u8,", name)?;
    }
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "unsafe impl Sync for ExtProc {{ }}")?;
    writeln!(f, "unsafe impl Send for ExtProc {{ }}")?;
    writeln!(f)?;
    writeln!(f, "impl ExtProc {{")?;
    writeln!(f, "    pub fn load() -> Self {{")?;
    writeln!(f, "        Self {{")?;
    for (name, _, _) in map.iter().copied() {
        writeln!(
            f,
            "            {}: unsafe {{ eglGetProcAddress(\"{}\\0\".as_ptr() as _) }},",
            name, name
        )?;
    }
    writeln!(f, "        }}")?;
    writeln!(f, "    }}")?;
    for (name, ret, args) in map.iter().copied() {
        let mut args_names = String::new();
        let mut args_full = String::new();
        let mut args_tys = String::new();
        for (name, ty) in args.iter().copied() {
            write!(args_full, "{}: {}, ", name, ty)?;
            write!(args_names, "{}, ", name)?;
            write!(args_tys, "{}, ", ty)?;
        }
        writeln!(f)?;
        writeln!(
            f,
            "    pub(super) unsafe fn {}(&self, {}) -> {} {{",
            name, args_full, ret
        )?;
        writeln!(f, "       if self.{}.is_null() {{", name)?;
        writeln!(f, "           panic!(\"Could not load `{}`\");", name)?;
        writeln!(f, "       }}")?;
        writeln!(
            f,
            "       ptr::read(&self.{} as *const *mut u8 as *const unsafe extern fn({}) -> {})({})",
            name, args_tys, ret, args_names
        )?;
        writeln!(f, "    }}")?;
    }
    writeln!(f, "}}")?;
    Ok(())
}

pub fn main() -> anyhow::Result<()> {
    let mut f = open("libinput_tys.rs")?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_LOG_PRIORITY,
        "libinput_log_priority",
    )?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_DEVICE_CAPABILITY,
        "libinput_device_capability",
    )?;
    write_ty(&mut f, libinput::LIBINPUT_KEY_STATE, "libinput_key_state")?;
    write_ty(&mut f, libinput::LIBINPUT_LED, "libinput_led")?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_BUTTON_STATE,
        "libinput_button_state",
    )?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_POINTER_AXIS,
        "libinput_pointer_axis",
    )?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_POINTER_AXIS_SOURCE,
        "libinput_pointer_axis_source",
    )?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_TABLET_PAD_RING_AXIS_SOURCE,
        "libinput_tablet_pad_ring_axis_source",
    )?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_TABLET_PAD_STRIP_AXIS_SOURCE,
        "libinput_tablet_pad_strip_axis_source",
    )?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_TABLET_TOOL_TYPE,
        "libinput_tablet_tool_type",
    )?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_TABLET_TOOL_PROXIMITY_STATE,
        "libinput_tablet_tool_proximity_state",
    )?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_TABLET_TOOL_TIP_STATE,
        "libinput_tablet_tool_tip_state",
    )?;
    write_ty(
        &mut f,
        libinput::LIBINPUT_SWITCH_STATE,
        "libinput_switch_state",
    )?;
    write_ty(&mut f, libinput::LIBINPUT_SWITCH, "libinput_switch")?;
    write_ty(&mut f, libinput::LIBINPUT_EVENT_TYPE, "libinput_event_type")?;

    let mut f = open("pixman_tys.rs")?;
    write_ty(&mut f, pixman::FORMATS, "PixmanFormat")?;
    write_ty(&mut f, pixman::OPS, "PixmanOp")?;

    let mut f = open("xkbcommon_tys.rs")?;
    write_ty(&mut f, xkbcommon::XKB_LOG_LEVEL, "xkb_log_level")?;
    write_ty(&mut f, xkbcommon::XKB_CONTEXT_FLAGS, "xkb_context_flags")?;
    write_ty(
        &mut f,
        xkbcommon::XKB_KEYMAP_COMPILE_FLAGS,
        "xkb_keymap_compile_flags",
    )?;
    write_ty(&mut f, xkbcommon::XKB_KEYMAP_FORMAT, "xkb_keymap_format")?;
    write_ty(
        &mut f,
        xkbcommon::XKB_STATE_COMPONENT,
        "xkb_state_component",
    )?;
    write_ty(&mut f, xkbcommon::XKB_KEY_DIRECTION, "xkb_key_direction")?;

    let mut f = open("egl_procs.rs")?;
    write_egl_procs(&mut f)?;

    Ok(())
}
