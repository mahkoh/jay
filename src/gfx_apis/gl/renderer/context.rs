use {
    crate::{
        format::{Format, XRGB8888},
        gfx_api::{
            BufferResvUser, GfxApiOpt, GfxContext, GfxError, GfxFormat, GfxFramebuffer, GfxImage,
            GfxTexture, ResetStatus,
        },
        gfx_apis::gl::{
            egl::{context::EglContext, display::EglDisplay, image::EglImage},
            ext::GL_OES_EGL_IMAGE_EXTERNAL,
            gl::{
                program::GlProgram, render_buffer::GlRenderBuffer, sys::GLint, texture::GlTexture,
            },
            renderer::{framebuffer::Framebuffer, image::Image},
            GfxGlState, RenderError, Texture,
        },
        video::{
            dmabuf::DmaBuf,
            drm::{sync_obj::SyncObjCtx, Drm},
            gbm::GbmDevice,
        },
    },
    ahash::AHashMap,
    enum_map::{enum_map, Enum, EnumMap},
    jay_config::video::GfxApi,
    std::{
        cell::{Cell, RefCell},
        ffi::CString,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
    uapi::ustr,
};

pub(crate) struct TexProg {
    pub(crate) prog: GlProgram,
    pub(crate) pos: GLint,
    pub(crate) texcoord: GLint,
    pub(crate) tex: GLint,
    pub(crate) alpha: GLint,
}

impl TexProg {
    unsafe fn from(prog: GlProgram, alpha_multiplier: bool) -> Self {
        let alpha = match alpha_multiplier {
            true => prog.get_uniform_location(ustr!("alpha")),
            false => 0,
        };
        Self {
            pos: prog.get_attrib_location(ustr!("pos")),
            texcoord: prog.get_attrib_location(ustr!("texcoord")),
            tex: prog.get_uniform_location(ustr!("tex")),
            alpha,
            prog,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Enum)]
pub(in crate::gfx_apis::gl) enum TexCopyType {
    Identity,
    Multiply,
}

#[derive(Copy, Clone, PartialEq, Enum)]
pub(in crate::gfx_apis::gl) enum TexSourceType {
    Opaque,
    HasAlpha,
}

pub(in crate::gfx_apis::gl) struct GlRenderContext {
    pub(crate) ctx: Rc<EglContext>,
    pub gbm: Rc<GbmDevice>,
    pub sync_ctx: Rc<SyncObjCtx>,

    pub(crate) render_node: Rc<CString>,

    pub(crate) tex_internal: EnumMap<TexCopyType, EnumMap<TexSourceType, TexProg>>,
    pub(crate) tex_external: Option<EnumMap<TexCopyType, EnumMap<TexSourceType, TexProg>>>,

    pub(crate) fill_prog: GlProgram,
    pub(crate) fill_prog_pos: GLint,
    pub(crate) fill_prog_color: GLint,

    pub(crate) gfx_ops: RefCell<Vec<GfxApiOpt>>,
    pub(in crate::gfx_apis::gl) gl_state: RefCell<GfxGlState>,

    pub(in crate::gfx_apis::gl) buffer_resv_user: BufferResvUser,
}

impl Debug for GlRenderContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderContext").finish_non_exhaustive()
    }
}

impl GlRenderContext {
    pub fn reset_status(&self) -> Option<ResetStatus> {
        self.ctx.reset_status()
    }

    pub(in crate::gfx_apis::gl) fn from_drm_device(drm: &Drm) -> Result<Self, RenderError> {
        let node = drm
            .get_render_node()?
            .ok_or(RenderError::NoRenderNode)
            .map(Rc::new)?;
        let dpy = EglDisplay::create(drm)?;
        if !dpy.formats.contains_key(&XRGB8888.drm) {
            return Err(RenderError::XRGB888);
        }
        let ctx = dpy.create_context()?;
        ctx.with_current(|| unsafe { Self::new(&ctx, &node) })
    }

    unsafe fn new(ctx: &Rc<EglContext>, node: &Rc<CString>) -> Result<Self, RenderError> {
        let tex_vert = include_str!("../shaders/tex.vert.glsl");
        let tex_frag = include_str!("../shaders/tex.frag.glsl");
        let create_programs = |external: bool| {
            let create_program = |alpha_multiplier: bool, alpha: bool| {
                let mut tex_frac_src = String::new();
                if external {
                    tex_frac_src.push_str("#define EXTERNAL\n");
                }
                if alpha_multiplier {
                    tex_frac_src.push_str("#define ALPHA_MULTIPLIER\n");
                }
                if alpha {
                    tex_frac_src.push_str("#define ALPHA\n");
                }
                tex_frac_src.push_str(tex_frag);
                let prog = GlProgram::from_shaders(ctx, tex_vert, &tex_frac_src)?;
                Ok::<_, RenderError>(TexProg::from(prog, alpha_multiplier))
            };
            Ok::<_, RenderError>(enum_map! {
                TexCopyType::Identity => enum_map! {
                    TexSourceType::Opaque => create_program(false, false)?,
                    TexSourceType::HasAlpha => create_program(false, true)?,
                },
                TexCopyType::Multiply => enum_map! {
                    TexSourceType::Opaque => create_program(true, false)?,
                    TexSourceType::HasAlpha => create_program(true, true)?,
                },
            })
        };
        let tex_internal = create_programs(false)?;
        let tex_external = if ctx.ext.contains(GL_OES_EGL_IMAGE_EXTERNAL) {
            Some(create_programs(true)?)
        } else {
            None
        };
        let fill_prog = GlProgram::from_shaders(
            ctx,
            include_str!("../shaders/fill.vert.glsl"),
            include_str!("../shaders/fill.frag.glsl"),
        )?;
        Ok(Self {
            ctx: ctx.clone(),
            gbm: ctx.dpy.gbm.clone(),
            sync_ctx: Rc::new(SyncObjCtx::new(ctx.dpy.gbm.drm.fd())),

            render_node: node.clone(),

            tex_internal,
            tex_external,

            fill_prog_pos: fill_prog.get_attrib_location(ustr!("pos")),
            fill_prog_color: fill_prog.get_uniform_location(ustr!("color")),
            fill_prog,

            gfx_ops: Default::default(),
            gl_state: Default::default(),

            buffer_resv_user: Default::default(),
        })
    }

    pub fn render_node(&self) -> Rc<CString> {
        self.render_node.clone()
    }

    pub fn formats(&self) -> Rc<AHashMap<u32, GfxFormat>> {
        self.ctx.formats.clone()
    }

    fn dmabuf_fb(self: &Rc<Self>, buf: &DmaBuf) -> Result<Rc<Framebuffer>, RenderError> {
        self.ctx.with_current(|| unsafe {
            let img = self.ctx.dpy.import_dmabuf(buf)?;
            let rb = GlRenderBuffer::from_image(&img, &self.ctx)?;
            let fb = rb.create_framebuffer()?;
            Ok(Rc::new(Framebuffer {
                ctx: self.clone(),
                gl: fb,
            }))
        })
    }

    fn dmabuf_img(self: &Rc<Self>, buf: &DmaBuf) -> Result<Rc<Image>, RenderError> {
        self.ctx.with_current(|| {
            let img = self.ctx.dpy.import_dmabuf(buf)?;
            Ok(Rc::new(Image {
                ctx: self.clone(),
                gl: img,
            }))
        })
    }

    fn shmem_texture(
        self: &Rc<Self>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> Result<Rc<Texture>, RenderError> {
        let gl = GlTexture::import_shm(&self.ctx, data, format, width, height, stride)?;
        Ok(Rc::new(Texture {
            ctx: self.clone(),
            gl,
            format,
        }))
    }

    pub fn image_to_fb(
        self: &Rc<Self>,
        img: &Rc<EglImage>,
    ) -> Result<Rc<Framebuffer>, RenderError> {
        self.ctx.with_current(|| unsafe {
            let rb = GlRenderBuffer::from_image(img, &self.ctx)?;
            let fb = rb.create_framebuffer()?;
            Ok(Rc::new(Framebuffer {
                ctx: self.clone(),
                gl: fb,
            }))
        })
    }
}

impl GfxContext for GlRenderContext {
    fn reset_status(&self) -> Option<ResetStatus> {
        self.reset_status()
    }

    fn render_node(&self) -> Rc<CString> {
        self.render_node()
    }

    fn formats(&self) -> Rc<AHashMap<u32, GfxFormat>> {
        self.formats()
    }

    fn dmabuf_fb(self: Rc<Self>, buf: &DmaBuf) -> Result<Rc<dyn GfxFramebuffer>, GfxError> {
        (&self)
            .dmabuf_fb(buf)
            .map(|w| w as Rc<dyn GfxFramebuffer>)
            .map_err(|e| e.into())
    }

    fn dmabuf_img(self: Rc<Self>, buf: &DmaBuf) -> Result<Rc<dyn GfxImage>, GfxError> {
        (&self)
            .dmabuf_img(buf)
            .map(|w| w as Rc<dyn GfxImage>)
            .map_err(|e| e.into())
    }

    fn shmem_texture(
        self: Rc<Self>,
        _old: Option<Rc<dyn GfxTexture>>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> Result<Rc<dyn GfxTexture>, GfxError> {
        (&self)
            .shmem_texture(data, format, width, height, stride)
            .map(|w| w as Rc<dyn GfxTexture>)
            .map_err(|e| e.into())
    }

    fn gbm(&self) -> &GbmDevice {
        &self.gbm
    }

    fn gfx_api(&self) -> GfxApi {
        GfxApi::OpenGl
    }

    fn create_fb(
        self: Rc<Self>,
        width: i32,
        height: i32,
        _stride: i32,
        format: &'static Format,
    ) -> Result<Rc<dyn GfxFramebuffer>, GfxError> {
        let fb = self.ctx.with_current(|| unsafe {
            GlRenderBuffer::new(&self.ctx, width, height, format)?.create_framebuffer()
        })?;
        Ok(Rc::new(Framebuffer { ctx: self, gl: fb }))
    }

    fn sync_obj_ctx(&self) -> &Rc<SyncObjCtx> {
        &self.sync_ctx
    }
}
