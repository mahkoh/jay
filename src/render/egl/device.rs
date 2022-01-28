use crate::format::{formats, Format};
use crate::render::egl::display::EglDisplay;
use crate::render::egl::sys::{
    eglInitialize, EGLDeviceEXT, EGLDisplay, EGLint, EGL_PLATFORM_DEVICE_EXT, EGL_TRUE,
};
use crate::render::egl::PROCS;
use crate::render::ext::{get_display_ext, DeviceExt, DisplayExt};
use crate::render::RenderError;
use ahash::AHashMap;
use std::ffi::CStr;
use std::ptr;
use std::rc::Rc;

#[derive(Debug, Copy, Clone)]
pub struct EglDevice {
    pub exts: DeviceExt,
    pub dev: EGLDeviceEXT,
}

impl EglDevice {
    pub fn query_string(&self, name: EGLint) -> Result<&'static CStr, RenderError> {
        unsafe {
            let res = PROCS.eglQueryDeviceStringEXT(self.dev, name);
            if res.is_null() {
                return Err(RenderError::DeviceQueryString);
            }
            Ok(CStr::from_ptr(res))
        }
    }

    pub fn create_display(&self) -> Result<Rc<EglDisplay>, RenderError> {
        unsafe {
            let dpy = PROCS.eglGetPlatformDisplayEXT(
                EGL_PLATFORM_DEVICE_EXT as _,
                self.dev.0,
                ptr::null(),
            );
            if dpy.is_none() {
                return Err(RenderError::GetDisplay);
            }
            let mut dpy = EglDisplay {
                exts: DisplayExt::empty(),
                formats: Rc::new(AHashMap::new()),
                dev: *self,
                dpy,
            };
            let mut major = 0;
            let mut minor = 0;
            if eglInitialize(dpy.dpy, &mut major, &mut minor) != EGL_TRUE {
                return Err(RenderError::Initialize);
            }
            dpy.exts = get_display_ext(dpy.dpy);
            if !dpy.exts.intersects(DisplayExt::KHR_IMAGE_BASE) {
                return Err(RenderError::ImageBase);
            }
            if !dpy
                .exts
                .intersects(DisplayExt::EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS)
            {
                return Err(RenderError::DmaBufImport);
            }
            if !dpy
                .exts
                .intersects(DisplayExt::KHR_NO_CONFIG_CONTEXT | DisplayExt::MESA_CONFIGLESS_CONTEXT)
            {
                return Err(RenderError::ConfiglessContext);
            }
            if !dpy.exts.intersects(DisplayExt::KHR_SURFACELESS_CONTEXT) {
                return Err(RenderError::SurfacelessContext);
            }
            dpy.formats = Rc::new(query_formats(dpy.dpy)?);

            Ok(Rc::new(dpy))
        }
    }
}

unsafe fn query_formats(dpy: EGLDisplay) -> Result<AHashMap<u32, &'static Format>, RenderError> {
    let mut vec = vec![];
    let mut num = 0;
    let res = PROCS.eglQueryDmaBufFormatsEXT(dpy, num, ptr::null_mut(), &mut num);
    if res != EGL_TRUE {
        return Err(RenderError::QueryDmaBufFormats);
    }
    vec.reserve_exact(num as usize);
    let res = PROCS.eglQueryDmaBufFormatsEXT(dpy, num, vec.as_mut_ptr(), &mut num);
    if res != EGL_TRUE {
        return Err(RenderError::QueryDmaBufFormats);
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
