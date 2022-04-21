use {
    crate::{
        format::{Format, XRGB8888},
        render::{
            egl::{
                context::EglContext,
                display::{EglDisplay, EglFormat},
            },
            gl::{
                program::GlProgram, render_buffer::GlRenderBuffer, sys::GLint, texture::GlTexture,
            },
            renderer::{framebuffer::Framebuffer, image::Image},
            RenderError, Texture,
        },
        video::{
            dmabuf::DmaBuf,
            drm::{Drm, NodeType},
            gbm::GbmDevice,
        },
    },
    ahash::AHashMap,
    std::{
        cell::Cell,
        ffi::CString,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
    uapi::ustr,
};

pub(super) struct TexProg {
    pub(super) prog: GlProgram,
    pub(super) pos: GLint,
    pub(super) texcoord: GLint,
    pub(super) tex: GLint,
}

impl TexProg {
    unsafe fn from(prog: GlProgram) -> Self {
        Self {
            pos: prog.get_attrib_location(ustr!("pos")),
            texcoord: prog.get_attrib_location(ustr!("texcoord")),
            tex: prog.get_uniform_location(ustr!("tex")),
            prog,
        }
    }
}

pub struct RenderContext {
    pub(super) ctx: Rc<EglContext>,
    pub gbm: Rc<GbmDevice>,

    pub(super) render_node: Rc<CString>,

    pub(super) tex_prog: TexProg,
    pub(super) tex_alpha_prog: TexProg,

    pub(super) fill_prog: GlProgram,
    pub(super) fill_prog_pos: GLint,
    pub(super) fill_prog_color: GLint,
}

impl Debug for RenderContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderContext").finish_non_exhaustive()
    }
}

impl RenderContext {
    pub fn from_drm_device(drm: &Drm) -> Result<Self, RenderError> {
        let nodes = drm.get_nodes()?;
        let node = match nodes
            .get(&NodeType::Render)
            .or_else(|| nodes.get(&NodeType::Primary))
        {
            None => return Err(RenderError::NoRenderNode),
            Some(path) => Rc::new(path.to_owned()),
        };
        let dpy = EglDisplay::create(drm)?;
        if !dpy.formats.contains_key(&XRGB8888.drm) {
            return Err(RenderError::XRGB888);
        }
        let ctx = dpy.create_context()?;
        ctx.with_current(|| unsafe { Self::new(&ctx, &node) })
    }

    unsafe fn new(ctx: &Rc<EglContext>, node: &Rc<CString>) -> Result<Self, RenderError> {
        let tex_prog = GlProgram::from_shaders(
            ctx,
            include_str!("../shaders/tex.vert.glsl"),
            include_str!("../shaders/tex.frag.glsl"),
        )?;
        let tex_alpha_prog = GlProgram::from_shaders(
            ctx,
            include_str!("../shaders/tex.vert.glsl"),
            include_str!("../shaders/tex-alpha.frag.glsl"),
        )?;
        let fill_prog = GlProgram::from_shaders(
            ctx,
            include_str!("../shaders/fill.vert.glsl"),
            include_str!("../shaders/fill.frag.glsl"),
        )?;
        Ok(Self {
            ctx: ctx.clone(),
            gbm: ctx.dpy.gbm.clone(),

            render_node: node.clone(),

            tex_prog: TexProg::from(tex_prog),
            tex_alpha_prog: TexProg::from(tex_alpha_prog),

            fill_prog_pos: fill_prog.get_attrib_location(ustr!("pos")),
            fill_prog_color: fill_prog.get_uniform_location(ustr!("color")),
            fill_prog,
        })
    }

    pub fn render_node(&self) -> Rc<CString> {
        self.render_node.clone()
    }

    pub fn formats(&self) -> Rc<AHashMap<u32, EglFormat>> {
        self.ctx.dpy.formats.clone()
    }

    pub fn dmabuf_fb(self: &Rc<Self>, buf: &DmaBuf) -> Result<Rc<Framebuffer>, RenderError> {
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

    pub fn dmabuf_img(self: &Rc<Self>, buf: &DmaBuf) -> Result<Rc<Image>, RenderError> {
        self.ctx.with_current(|| {
            let img = self.ctx.dpy.import_dmabuf(buf)?;
            Ok(Rc::new(Image {
                ctx: self.clone(),
                gl: img,
            }))
        })
    }

    pub fn shmem_texture(
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
        }))
    }
}
