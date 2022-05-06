use {
    crate::open,
    std::{fmt::Write as FmtWrite, io::Write},
};

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
    ];

    writeln!(f, "use std::ptr;")?;
    writeln!(f, "use super::gl::sys::*;")?;
    writeln!(f, "use super::egl::sys::*;")?;
    writeln!(f)?;
    writeln!(f, "#[derive(Copy, Clone, Debug)]")?;
    writeln!(f, "pub struct ExtProc {{")?;
    for (name, _, _) in map.iter() {
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
    let mut f = open("egl_procs.rs")?;
    write_egl_procs(&mut f)?;

    Ok(())
}
