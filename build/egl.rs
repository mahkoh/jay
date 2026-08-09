use crate::open;
use std::fmt::Write as FmtWrite;
use std::io::Write;

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
            "glEGLImageTargetRenderbufferStorageOES",
            "()",
            &[("target", "GLenum"), ("image", "GLeglImageOES")][..],
        ),
        (
            "glEGLImageTargetTexture2DOES",
            "()",
            &[("target", "GLenum"), ("image", "GLeglImageOES")][..],
        ),
        ("glGetGraphicsResetStatusKHR", "GLenum", &[][..]),
        (
            "eglCreateSyncKHR",
            "EGLSyncKHR",
            &[
                ("dpy", "EGLDisplay"),
                ("ty", "EGLenum"),
                ("attrib_list", "*const EGLint"),
            ][..],
        ),
        (
            "eglDestroySyncKHR",
            "EGLBoolean",
            &[("dpy", "EGLDisplay"), ("sync", "EGLSyncKHR")][..],
        ),
        (
            "eglDupNativeFenceFDANDROID",
            "EGLint",
            &[("dpy", "EGLDisplay"), ("sync", "EGLSyncKHR")][..],
        ),
        (
            "eglWaitSyncKHR",
            "EGLint",
            &[
                ("dpy", "EGLDisplay"),
                ("sync", "EGLSyncKHR"),
                ("flags", "EGLint"),
            ][..],
        ),
        (
            "eglQueryDeviceStringEXT",
            "*const c::c_char",
            &[("device", "EGLDeviceEXT"), ("name", "EGLint")][..],
        ),
    ];

    define_w!(f, w, wl);
    define_xn!(xn);
    wl!("use std::ptr;");
    wl!("use super::gl::sys::*;");
    wl!("use super::egl::sys::*;");
    wl!();
    wl!("#[derive(Copy, Clone, Debug)]");
    wl!("pub struct ExtProc {{");
    {
        push_xn!(xn);
        for (name, _, _) in map.iter() {
            wl!("{xn}{}: *mut u8,", name);
        }
    }
    wl!("}}");
    wl!();
    wl!("unsafe impl Sync for ExtProc {{ }}");
    wl!("unsafe impl Send for ExtProc {{ }}");
    wl!();
    wl!("impl ExtProc {{");
    {
        push_xn!(xn);
        wl!("{xn}pub fn load() -> Option<Self> {{");
        {
            push_xn!(xn);
            wl!("{xn}let egl = EGL.as_ref()?;");
            wl!("{xn}Some(Self {{");
            {
                push_xn!(xn);
                for (name, _, _) in map.iter().copied() {
                    wl!(
                        "{xn}{}: unsafe {{ (egl.eglGetProcAddress)(c\"{}\".as_ptr() as _) }},",
                        name,
                        name
                    );
                }
            }
            wl!("{xn}}})");
        }
        wl!("{xn}}}");
        for (name, ret, args) in map.iter().copied() {
            let mut args_names = String::new();
            let mut args_full = String::new();
            let mut args_tys = String::new();
            for (name, ty) in args.iter().copied() {
                write!(args_full, "{}: {}, ", name, ty)?;
                write!(args_names, "{}, ", name)?;
                write!(args_tys, "{}, ", ty)?;
            }
            wl!();
            wl!(
                "{xn}pub(super) unsafe fn {}(&self, {}) -> {} {{",
                name,
                args_full,
                ret
            );
            {
                push_xn!(xn);
                wl!("{xn}if self.{}.is_null() {{", name);
                {
                    push_xn!(xn);
                    wl!("{xn}panic!(\"Could not load `{}`\");", name);
                }
                wl!("{xn}}}");
                wl!("{xn}unsafe {{");
                {
                    push_xn!(xn);
                    wl!(
                        r#"{xn}ptr::read(&self.{} as *const *mut u8 as *const unsafe extern "C" fn({}) -> {})({})"#,
                        name,
                        args_tys,
                        ret,
                        args_names
                    );
                }
                wl!("{xn}}}");
            }
            wl!("{xn}}}");
        }
    }
    wl!("}}");
    Ok(())
}

pub fn main() -> anyhow::Result<()> {
    let mut f = open("egl_procs.rs")?;
    write_egl_procs(&mut f)?;

    Ok(())
}
