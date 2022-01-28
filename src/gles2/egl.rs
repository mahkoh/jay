use super::ext::{get_client_ext, ClientExt};
use super::ext_proc::ExtProc;
use super::sys::{
    eglBindAPI, EGLAttrib, EGLLabelKHR, EGLenum, EGLint, EGL_BAD_ACCESS, EGL_BAD_ALLOC,
    EGL_BAD_ATTRIBUTE, EGL_BAD_CONFIG, EGL_BAD_CONTEXT, EGL_BAD_CURRENT_SURFACE,
    EGL_BAD_DEVICE_EXT, EGL_BAD_DISPLAY, EGL_BAD_MATCH, EGL_BAD_NATIVE_PIXMAP,
    EGL_BAD_NATIVE_WINDOW, EGL_BAD_PARAMETER, EGL_BAD_SURFACE, EGL_CONTEXT_LOST,
    EGL_DEBUG_MSG_CRITICAL_KHR, EGL_DEBUG_MSG_ERROR_KHR, EGL_DEBUG_MSG_INFO_KHR,
    EGL_DEBUG_MSG_WARN_KHR, EGL_NONE, EGL_NOT_INITIALIZED, EGL_OPENGL_ES_API, EGL_SUCCESS,
    EGL_TRUE,
};
use super::GlesError;
use crate::drm::dma::DmaBuf;
use crate::drm::drm::Drm;
use crate::drm::INVALID_MODIFIER;
use crate::format::{formats, Format};
use crate::gles2::ext::{
    get_device_ext, get_display_ext, get_gl_ext, DeviceExt, DisplayExt, GlExt,
};
use crate::gles2::gl::GlTexture;
use crate::gles2::sys::{
    eglCreateContext, eglDestroyContext, eglInitialize, eglMakeCurrent, eglTerminate,
    glBindFramebuffer, glBindRenderbuffer, glBindTexture, glCheckFramebufferStatus, glClear,
    glClearColor, glDeleteFramebuffers, glDeleteRenderbuffers, glFlush, glFramebufferRenderbuffer,
    glGenFramebuffers, glGenRenderbuffers, glGenTextures, glPixelStorei, glTexImage2D,
    glTexParameteri, EGLClientBuffer, EGLConfig, EGLContext, EGLDeviceEXT, EGLDisplay, EGLImageKHR,
    EGLSurface, GLeglImageOES, GLint, GLuint, EGL_CONTEXT_CLIENT_VERSION,
    EGL_DMA_BUF_PLANE0_FD_EXT, EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
    EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT, EGL_DMA_BUF_PLANE0_OFFSET_EXT,
    EGL_DMA_BUF_PLANE0_PITCH_EXT, EGL_DMA_BUF_PLANE1_FD_EXT, EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT,
    EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT, EGL_DMA_BUF_PLANE1_OFFSET_EXT,
    EGL_DMA_BUF_PLANE1_PITCH_EXT, EGL_DMA_BUF_PLANE2_FD_EXT, EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT,
    EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT, EGL_DMA_BUF_PLANE2_OFFSET_EXT,
    EGL_DMA_BUF_PLANE2_PITCH_EXT, EGL_DMA_BUF_PLANE3_FD_EXT, EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT,
    EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT, EGL_DMA_BUF_PLANE3_OFFSET_EXT,
    EGL_DMA_BUF_PLANE3_PITCH_EXT, EGL_DRM_DEVICE_FILE_EXT, EGL_FALSE, EGL_HEIGHT,
    EGL_IMAGE_PRESERVED_KHR, EGL_LINUX_DMA_BUF_EXT, EGL_LINUX_DRM_FOURCC_EXT,
    EGL_PLATFORM_DEVICE_EXT, EGL_WIDTH, GL_CLAMP_TO_EDGE, GL_COLOR_ATTACHMENT0,
    GL_COLOR_BUFFER_BIT, GL_FRAMEBUFFER, GL_FRAMEBUFFER_COMPLETE, GL_RENDERBUFFER, GL_TEXTURE_2D,
    GL_TEXTURE_WRAP_S, GL_TEXTURE_WRAP_T, GL_UNPACK_ROW_LENGTH_EXT,
};
use crate::rect::Rect;
use ahash::AHashMap;
use bstr::ByteSlice;
use log::Level;
use once_cell::sync::Lazy;
use std::cell::Cell;
use std::ffi::CStr;
use std::ptr;
use std::rc::Rc;
use uapi::c;

#[thread_local]
pub(super) static PROCS: Lazy<ExtProc> = Lazy::new(|| ExtProc::load());

#[thread_local]
pub(super) static EXTS: Lazy<ClientExt> = Lazy::new(|| get_client_ext());

pub fn init() -> Result<(), GlesError> {
    if !EXTS.contains(ClientExt::EXT_PLATFORM_BASE) {
        return Err(GlesError::ExtPlatformBase);
    }
    if !EXTS.device_query() {
        return Err(GlesError::DeviceQuery);
    }
    if !EXTS.device_enumeration() {
        return Err(GlesError::DeviceEnumeration);
    }
    if EXTS.contains(ClientExt::KHR_DEBUG) {
        let attrib: &[EGLAttrib] = &[
            EGL_DEBUG_MSG_CRITICAL_KHR as _,
            EGL_TRUE as _,
            EGL_DEBUG_MSG_ERROR_KHR as _,
            EGL_TRUE as _,
            EGL_DEBUG_MSG_WARN_KHR as _,
            EGL_TRUE as _,
            EGL_DEBUG_MSG_INFO_KHR as _,
            EGL_TRUE as _,
            EGL_NONE as _,
        ];
        unsafe {
            PROCS.eglDebugMessageControlKHR(egl_log, attrib.as_ptr());
        }
    }
    if unsafe { eglBindAPI(EGL_OPENGL_ES_API) } != EGL_TRUE {
        return Err(GlesError::BindFailed);
    }
    Ok(())
}

pub fn find_drm_device(drm: &Drm) -> Result<Option<EglDevice>, GlesError> {
    let drm_dev = drm.get_device()?;
    for device in query_devices()? {
        if device.exts.contains(DeviceExt::EXT_DEVICE_DRM) {
            let device_file = device.query_string(EGL_DRM_DEVICE_FILE_EXT)?;
            for (_, name) in drm_dev.nodes() {
                if device_file == name {
                    return Ok(Some(device));
                }
            }
        }
    }
    Ok(None)
}

pub fn query_devices() -> Result<Vec<EglDevice>, GlesError> {
    if !EXTS.device_enumeration() {
        return Err(GlesError::DeviceEnumeration);
    }
    unsafe {
        let mut devices = vec![];
        let mut num_devices = 0;
        let res = PROCS.eglQueryDevicesEXT(num_devices, ptr::null_mut(), &mut num_devices);
        if res != EGL_TRUE {
            return Err(GlesError::QueryDevices);
        }
        devices.reserve_exact(num_devices as usize);
        let res = PROCS.eglQueryDevicesEXT(num_devices, devices.as_mut_ptr(), &mut num_devices);
        if res != EGL_TRUE {
            return Err(GlesError::QueryDevices);
        }
        devices.set_len(num_devices as usize);
        Ok(devices
            .into_iter()
            .map(|d| EglDevice {
                exts: get_device_ext(d),
                dev: d,
            })
            .collect())
    }
}

#[derive(Debug, Copy, Clone)]
pub struct EglDevice {
    pub exts: DeviceExt,
    dev: EGLDeviceEXT,
}

#[derive(Debug, Clone)]
pub struct EglDisplay {
    pub exts: DisplayExt,
    pub formats: AHashMap<u32, &'static Format>,
    dev: EglDevice,
    dpy: EGLDisplay,
}

#[derive(Debug, Clone)]
pub struct EglContext {
    pub dpy: Rc<EglDisplay>,
    pub ext: GlExt,
    ctx: EGLContext,

    pub tex_prog: GLuint,
    pub tex_tex: GLint,
    pub tex_texcoord: GLint,
    pub tex_pos: GLint,
}

impl EglDisplay {
    pub fn create_context(self: &Rc<Self>) -> Result<Rc<EglContext>, GlesError> {
        let attrib = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE];
        unsafe {
            let ctx = eglCreateContext(
                self.dpy,
                EGLConfig::none(),
                EGLContext::none(),
                attrib.as_ptr(),
            );
            if ctx.is_none() {
                return Err(GlesError::CreateContext);
            }
            let mut ctx = EglContext {
                dpy: self.clone(),
                ext: GlExt::empty(),
                ctx,
                tex_prog: 0,
                tex_tex: 0,
                tex_texcoord: 0,
                tex_pos: 0,
            };
            ctx.ext = ctx.with_current(|| Ok(get_gl_ext()))?;
            // if !ctx.ext.contains(GlExt::GL_OES_EGL_IMAGE) {
            //     return Err(GlesError::OesEglImage);
            // }
            Ok(Rc::new(ctx))
        }
    }

    pub fn import_dmabuf(self: &Rc<Self>, buf: &DmaBuf) -> Result<Rc<EglImage>, GlesError> {
        struct PlaneKey {
            fd: EGLint,
            offset: EGLint,
            pitch: EGLint,
            mod_lo: EGLint,
            mod_hi: EGLint,
        }
        const PLANE_KEYS: [PlaneKey; 4] = [
            PlaneKey {
                fd: EGL_DMA_BUF_PLANE0_FD_EXT,
                offset: EGL_DMA_BUF_PLANE0_OFFSET_EXT,
                pitch: EGL_DMA_BUF_PLANE0_PITCH_EXT,
                mod_lo: EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
                mod_hi: EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
            },
            PlaneKey {
                fd: EGL_DMA_BUF_PLANE1_FD_EXT,
                offset: EGL_DMA_BUF_PLANE1_OFFSET_EXT,
                pitch: EGL_DMA_BUF_PLANE1_PITCH_EXT,
                mod_lo: EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT,
                mod_hi: EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT,
            },
            PlaneKey {
                fd: EGL_DMA_BUF_PLANE2_FD_EXT,
                offset: EGL_DMA_BUF_PLANE2_OFFSET_EXT,
                pitch: EGL_DMA_BUF_PLANE2_PITCH_EXT,
                mod_lo: EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT,
                mod_hi: EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT,
            },
            PlaneKey {
                fd: EGL_DMA_BUF_PLANE3_FD_EXT,
                offset: EGL_DMA_BUF_PLANE3_OFFSET_EXT,
                pitch: EGL_DMA_BUF_PLANE3_PITCH_EXT,
                mod_lo: EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT,
                mod_hi: EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT,
            },
        ];

        let mut attribs = vec![];
        attribs.extend_from_slice(&[EGL_WIDTH, buf.width]);
        attribs.extend_from_slice(&[EGL_HEIGHT, buf.height]);
        attribs.extend_from_slice(&[EGL_LINUX_DRM_FOURCC_EXT, buf.format.drm as _]);
        attribs.extend_from_slice(&[EGL_IMAGE_PRESERVED_KHR, EGL_TRUE as _]);
        for (key, plane) in PLANE_KEYS.iter().zip(buf.planes.iter()) {
            attribs.extend_from_slice(&[key.fd, plane.fd.raw()]);
            attribs.extend_from_slice(&[key.pitch, plane.stride as _]);
            attribs.extend_from_slice(&[key.offset, plane.offset as _]);
            if buf.modifier != INVALID_MODIFIER {
                attribs.extend_from_slice(&[key.mod_lo, buf.modifier as i32]);
                attribs.extend_from_slice(&[key.mod_hi, (buf.modifier >> 32) as i32]);
            }
        }
        attribs.push(EGL_NONE);
        let img = unsafe {
            PROCS.eglCreateImageKHR(
                self.dpy,
                EGLContext::none(),
                EGL_LINUX_DMA_BUF_EXT as _,
                EGLClientBuffer::none(),
                attribs.as_ptr(),
            )
        };
        if img.is_none() {
            return Err(GlesError::CreateImage);
        }
        Ok(Rc::new(EglImage {
            dpy: self.clone(),
            img,
            width: buf.width,
            height: buf.height,
        }))
    }
}

pub struct EglImage {
    dpy: Rc<EglDisplay>,
    pub(super) img: EGLImageKHR,
    pub width: i32,
    pub height: i32,
}

impl Drop for EglImage {
    fn drop(&mut self) {
        unsafe {
            if PROCS.eglDestroyImageKHR(self.dpy.dpy, self.img) == EGL_FALSE {
                log::warn!("`eglDestroyImageKHR` failed");
            }
        }
    }
}

impl Drop for EglContext {
    fn drop(&mut self) {
        unsafe {
            if eglDestroyContext(self.dpy.dpy, self.ctx) != EGL_TRUE {
                log::warn!("`eglDestroyContext` failed");
            }
        }
    }
}

#[thread_local]
static mut CURRENT: EGLContext = EGLContext::none();

impl EglContext {
    #[inline]
    pub fn with_current<T, F: FnOnce() -> Result<T, GlesError>>(
        &self,
        f: F,
    ) -> Result<T, GlesError> {
        unsafe {
            if CURRENT == self.ctx {
                return f();
            }
            self.with_current_slow(f)
        }
    }

    #[cold]
    unsafe fn with_current_slow<T, F: FnOnce() -> Result<T, GlesError>>(
        &self,
        f: F,
    ) -> Result<T, GlesError> {
        if eglMakeCurrent(
            self.dpy.dpy,
            EGLSurface::none(),
            EGLSurface::none(),
            self.ctx,
        ) == EGL_FALSE
        {
            return Err(GlesError::MakeCurrent);
        }
        let prev = CURRENT;
        CURRENT = self.ctx;
        let res = f();
        if eglMakeCurrent(self.dpy.dpy, EGLSurface::none(), EGLSurface::none(), prev) == EGL_FALSE {
            panic!("Could not restore EGLContext");
        }
        CURRENT = prev;
        res
    }
}

impl Drop for EglDisplay {
    fn drop(&mut self) {
        unsafe {
            if eglTerminate(self.dpy) != EGL_TRUE {
                log::warn!("`eglTerminate` failed");
            }
        }
    }
}

impl EglDevice {
    pub fn query_string(&self, name: EGLint) -> Result<&'static CStr, GlesError> {
        unsafe {
            let res = PROCS.eglQueryDeviceStringEXT(self.dev, name);
            if res.is_null() {
                return Err(GlesError::DeviceQueryString);
            }
            Ok(CStr::from_ptr(res))
        }
    }

    pub fn create_display(&self) -> Result<Rc<EglDisplay>, GlesError> {
        unsafe {
            let dpy = PROCS.eglGetPlatformDisplayEXT(
                EGL_PLATFORM_DEVICE_EXT as _,
                self.dev.0,
                ptr::null(),
            );
            if dpy.is_none() {
                return Err(GlesError::GetDisplay);
            }
            let mut dpy = EglDisplay {
                exts: DisplayExt::empty(),
                formats: AHashMap::new(),
                dev: *self,
                dpy,
            };
            let mut major = 0;
            let mut minor = 0;
            if eglInitialize(dpy.dpy, &mut major, &mut minor) != EGL_TRUE {
                return Err(GlesError::Initialize);
            }
            dpy.exts = get_display_ext(dpy.dpy);
            if !dpy.exts.intersects(DisplayExt::KHR_IMAGE_BASE) {
                return Err(GlesError::ImageBase);
            }
            if !dpy
                .exts
                .intersects(DisplayExt::EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS)
            {
                return Err(GlesError::DmaBufImport);
            }
            if !dpy
                .exts
                .intersects(DisplayExt::KHR_NO_CONFIG_CONTEXT | DisplayExt::MESA_CONFIGLESS_CONTEXT)
            {
                return Err(GlesError::ConfiglessContext);
            }
            if !dpy.exts.intersects(DisplayExt::KHR_SURFACELESS_CONTEXT) {
                return Err(GlesError::SurfacelessContext);
            }
            dpy.formats = query_formats(dpy.dpy)?;

            Ok(Rc::new(dpy))
        }
    }
}

unsafe fn query_formats(dpy: EGLDisplay) -> Result<AHashMap<u32, &'static Format>, GlesError> {
    let mut vec = vec![];
    let mut num = 0;
    let res = PROCS.eglQueryDmaBufFormatsEXT(dpy, num, ptr::null_mut(), &mut num);
    if res != EGL_TRUE {
        return Err(GlesError::QueryDmaBufFormats);
    }
    vec.reserve_exact(num as usize);
    let res = PROCS.eglQueryDmaBufFormatsEXT(dpy, num, vec.as_mut_ptr(), &mut num);
    if res != EGL_TRUE {
        return Err(GlesError::QueryDmaBufFormats);
    }
    vec.set_len(num as usize);
    let mut res = AHashMap::new();
    let formats = formats();
    for fmt in vec {
        if let Some(format) = formats.get(&(fmt as u32)) {
            res.insert(format.drm, *format);
        }
    }
    Ok(res)
}

unsafe extern "C" fn egl_log(
    error: EGLenum,
    command: *const c::c_char,
    message_type: EGLint,
    _thread_label: EGLLabelKHR,
    _object_label: EGLLabelKHR,
    message: *const c::c_char,
) {
    let level = match message_type {
        EGL_DEBUG_MSG_CRITICAL_KHR => Level::Error,
        EGL_DEBUG_MSG_ERROR_KHR => Level::Error,
        EGL_DEBUG_MSG_WARN_KHR => Level::Warn,
        EGL_DEBUG_MSG_INFO_KHR => Level::Info,
        _ => Level::Warn,
    };
    let command = if !command.is_null() {
        CStr::from_ptr(command).to_bytes()
    } else {
        b"none"
    };
    let message = if !message.is_null() {
        CStr::from_ptr(message).to_bytes()
    } else {
        b"none"
    };
    let err_name = error_name(error);
    log::log!(
        level,
        "EGL: command: {}, error: {} (0x{:x}), message: {}",
        command.as_bstr(),
        err_name,
        error,
        message.as_bstr()
    );
}

fn error_name(error: EGLenum) -> &'static str {
    macro_rules! en {
        ($($name:ident,)*) => {
            match error as _ {
                $($name => stringify!($name),)*
                _ => "unknown",
            }
        }
    }
    en! {
        EGL_SUCCESS,
        EGL_NOT_INITIALIZED,
        EGL_BAD_ACCESS,
        EGL_BAD_ALLOC,
        EGL_BAD_ATTRIBUTE,
        EGL_BAD_CONTEXT,
        EGL_BAD_CONFIG,
        EGL_BAD_CURRENT_SURFACE,
        EGL_BAD_DISPLAY,
        EGL_BAD_DEVICE_EXT,
        EGL_BAD_SURFACE,
        EGL_BAD_MATCH,
        EGL_BAD_PARAMETER,
        EGL_BAD_NATIVE_PIXMAP,
        EGL_BAD_NATIVE_WINDOW,
        EGL_CONTEXT_LOST,
    }
}
