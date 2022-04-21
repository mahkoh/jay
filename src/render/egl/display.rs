use {
    crate::{
        format::{formats, Format},
        render::{
            egl::{
                context::EglContext,
                image::EglImage,
                sys::{
                    eglCreateContext, eglTerminate, EGLClientBuffer, EGLConfig, EGLContext,
                    EGLDisplay, EGLint, EGL_CONTEXT_CLIENT_VERSION, EGL_DMA_BUF_PLANE0_FD_EXT,
                    EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT, EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
                    EGL_DMA_BUF_PLANE0_OFFSET_EXT, EGL_DMA_BUF_PLANE0_PITCH_EXT,
                    EGL_DMA_BUF_PLANE1_FD_EXT, EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT,
                    EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT, EGL_DMA_BUF_PLANE1_OFFSET_EXT,
                    EGL_DMA_BUF_PLANE1_PITCH_EXT, EGL_DMA_BUF_PLANE2_FD_EXT,
                    EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT, EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT,
                    EGL_DMA_BUF_PLANE2_OFFSET_EXT, EGL_DMA_BUF_PLANE2_PITCH_EXT,
                    EGL_DMA_BUF_PLANE3_FD_EXT, EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT,
                    EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT, EGL_DMA_BUF_PLANE3_OFFSET_EXT,
                    EGL_DMA_BUF_PLANE3_PITCH_EXT, EGL_HEIGHT, EGL_IMAGE_PRESERVED_KHR,
                    EGL_LINUX_DMA_BUF_EXT, EGL_LINUX_DRM_FOURCC_EXT, EGL_NONE, EGL_TRUE, EGL_WIDTH,
                },
                PROCS,
            },
            ext::{get_display_ext, get_gl_ext, DisplayExt, GlExt},
            sys::{eglInitialize, EGL_PLATFORM_GBM_KHR},
            RenderError,
        },
        video::{dmabuf::DmaBuf, drm::Drm, gbm::GbmDevice, INVALID_MODIFIER, LINEAR_MODIFIER},
    },
    ahash::AHashMap,
    std::{ptr, rc::Rc},
};

#[derive(Debug)]
pub struct EglFormat {
    pub format: &'static Format,
    pub modifiers: AHashMap<u64, EglModifier>,
}

#[derive(Debug)]
pub struct EglModifier {
    pub modifier: u64,
    pub external_only: bool,
}

#[derive(Debug)]
pub struct EglDisplay {
    pub exts: DisplayExt,
    pub formats: Rc<AHashMap<u32, EglFormat>>,
    pub gbm: Rc<GbmDevice>,
    pub dpy: EGLDisplay,
}

impl EglDisplay {
    pub fn create(drm: &Drm) -> Result<Rc<Self>, RenderError> {
        unsafe {
            let gbm = match GbmDevice::new(drm) {
                Ok(gbm) => gbm,
                Err(e) => return Err(RenderError::Gbm(e)),
            };
            let dpy = PROCS.eglGetPlatformDisplayEXT(
                EGL_PLATFORM_GBM_KHR as _,
                gbm.raw() as _,
                ptr::null(),
            );
            if dpy.is_none() {
                return Err(RenderError::GetDisplay);
            }
            let mut dpy = EglDisplay {
                exts: DisplayExt::empty(),
                formats: Rc::new(AHashMap::new()),
                gbm: Rc::new(gbm),
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

    pub fn create_context(self: &Rc<Self>) -> Result<Rc<EglContext>, RenderError> {
        let attrib = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE];
        unsafe {
            let ctx = eglCreateContext(
                self.dpy,
                EGLConfig::none(),
                EGLContext::none(),
                attrib.as_ptr(),
            );
            if ctx.is_none() {
                return Err(RenderError::CreateContext);
            }
            let mut ctx = EglContext {
                dpy: self.clone(),
                ext: GlExt::empty(),
                ctx,
            };
            ctx.ext = ctx.with_current(|| Ok(get_gl_ext()))?;
            if !ctx.ext.contains(GlExt::GL_OES_EGL_IMAGE) {
                return Err(RenderError::OesEglImage);
            }
            Ok(Rc::new(ctx))
        }
    }

    pub fn import_dmabuf(self: &Rc<Self>, buf: &DmaBuf) -> Result<Rc<EglImage>, RenderError> {
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

        let mut attribs = Vec::with_capacity(19);
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
            return Err(RenderError::CreateImage);
        }
        Ok(Rc::new(EglImage {
            dpy: self.clone(),
            img,
            width: buf.width,
            height: buf.height,
        }))
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

unsafe fn query_formats(dpy: EGLDisplay) -> Result<AHashMap<u32, EglFormat>, RenderError> {
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
            res.insert(
                format.drm,
                EglFormat {
                    format: *format,
                    modifiers: query_modifiers(dpy, fmt)?,
                },
            );
        }
    }
    Ok(res)
}

unsafe fn query_modifiers(
    dpy: EGLDisplay,
    format: EGLint,
) -> Result<AHashMap<u64, EglModifier>, RenderError> {
    let mut mods = vec![];
    let mut ext_only = vec![];
    let mut num = 0;
    let res = PROCS.eglQueryDmaBufModifiersEXT(
        dpy,
        format,
        num,
        ptr::null_mut(),
        ptr::null_mut(),
        &mut num,
    );
    if res != EGL_TRUE {
        return Err(RenderError::QueryDmaBufModifiers);
    }
    mods.reserve_exact(num as usize);
    ext_only.reserve_exact(num as usize);
    let res = PROCS.eglQueryDmaBufModifiersEXT(
        dpy,
        format,
        num,
        mods.as_mut_ptr(),
        ext_only.as_mut_ptr(),
        &mut num,
    );
    if res != EGL_TRUE {
        return Err(RenderError::QueryDmaBufModifiers);
    }
    mods.set_len(num as usize);
    ext_only.set_len(num as usize);
    let mut res = AHashMap::new();
    res.insert(
        INVALID_MODIFIER,
        EglModifier {
            modifier: INVALID_MODIFIER,
            external_only: false,
        },
    );
    if mods.is_empty() {
        res.insert(
            LINEAR_MODIFIER,
            EglModifier {
                modifier: LINEAR_MODIFIER,
                external_only: false,
            },
        );
    }
    for (modifier, ext_only) in mods.iter().copied().zip(ext_only.iter().copied()) {
        res.insert(
            modifier as _,
            EglModifier {
                modifier: modifier as _,
                external_only: ext_only == EGL_TRUE,
            },
        );
    }
    Ok(res)
}
