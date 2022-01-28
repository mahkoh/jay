use crate::egl::{EglContext, EglImage, PROCS};
use crate::format::Format;
use crate::gles2::sys::{
    glAttachShader, glBindFramebuffer, glBindRenderbuffer, glBindTexture, glCheckFramebufferStatus,
    glClear, glClearColor, glCompileShader, glCreateProgram, glCreateShader, glDeleteFramebuffers,
    glDeleteProgram, glDeleteRenderbuffers, glDeleteShader, glDeleteTextures, glDetachShader,
    glDisable, glEnable, glFramebufferRenderbuffer, glFramebufferTexture2D, glGenFramebuffers,
    glGenRenderbuffers, glGenTextures, glGetAttribLocation, glGetProgramiv, glGetShaderiv,
    glGetUniformLocation, glLinkProgram, glPixelStorei, glScissor, glShaderSource, glTexImage2D,
    glTexParameteri, glUseProgram, EGLContext, GLeglImageOES, GLenum, GLint, GLuint,
    GL_CLAMP_TO_EDGE, GL_COLOR_ATTACHMENT0, GL_COLOR_BUFFER_BIT, GL_COMPILE_STATUS, GL_FALSE,
    GL_FRAMEBUFFER, GL_FRAMEBUFFER_COMPLETE, GL_LINEAR, GL_LINK_STATUS, GL_RENDERBUFFER,
    GL_SCISSOR_TEST, GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_TEXTURE_MIN_FILTER,
    GL_TEXTURE_WRAP_S, GL_TEXTURE_WRAP_T, GL_UNPACK_ROW_LENGTH_EXT,
};
use crate::gles2::GlesError;
use crate::rect::Rect;
use crate::utils::ptr_ext::PtrExt;
use std::cell::Cell;
use std::ptr;
use std::rc::Rc;
use uapi::{ustr, Ustr};

pub struct GlTexture {
    pub(super) ctx: Rc<EglContext>,
    pub tex: GLuint,
    pub width: i32,
    pub height: i32,
}

impl GlTexture {
    pub fn new(
        ctx: &Rc<EglContext>,
        format: &'static Format,
        width: i32,
        height: i32,
    ) -> Result<Rc<GlTexture>, GlesError> {
        let tex = ctx.with_current(|| unsafe {
            let mut tex = 0;
            glGenTextures(1, &mut tex);
            glBindTexture(GL_TEXTURE_2D, tex);
            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                format.gl_format,
                width,
                height,
                0,
                format.gl_format as _,
                format.gl_type as _,
                ptr::null(),
            );
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            glBindTexture(GL_TEXTURE_2D, 0);
            Ok(tex)
        })?;
        Ok(Rc::new(GlTexture {
            ctx: ctx.clone(),
            tex,
            width,
            height,
        }))
    }

    pub unsafe fn to_framebuffer(self: &Rc<Self>) -> Result<Rc<GlFrameBuffer>, GlesError> {
        self.ctx.with_current(|| unsafe {
            let mut fbo = 0;
            glGenFramebuffers(1, &mut fbo);
            glBindFramebuffer(GL_FRAMEBUFFER, fbo);
            glFramebufferTexture2D(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                self.tex,
                0,
            );
            let fb = GlFrameBuffer {
                _rb: None,
                _tex: Some(self.clone()),
                ctx: self.ctx.clone(),
                fbo,
                width: self.width,
                height: self.height,
            };
            let status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
            if status != GL_FRAMEBUFFER_COMPLETE {
                return Err(GlesError::CreateFramebuffer);
            }
            Ok(Rc::new(fb))
        })
    }

    pub fn import_texture(
        ctx: &Rc<EglContext>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> Result<Rc<GlTexture>, GlesError> {
        if (stride * height) as usize > data.len() {
            return Err(GlesError::SmallImageBuffer);
        }
        let tex = ctx.with_current(|| unsafe {
            let mut tex = 0;
            glGenTextures(1, &mut tex);
            glBindTexture(GL_TEXTURE_2D, tex);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            glPixelStorei(GL_UNPACK_ROW_LENGTH_EXT, stride / format.bpp as GLint);
            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                format.gl_format,
                width,
                height,
                0,
                format.gl_format as _,
                format.gl_type as _,
                data.as_ptr() as _,
            );
            glPixelStorei(GL_UNPACK_ROW_LENGTH_EXT, 0);
            glBindTexture(GL_TEXTURE_2D, 0);
            Ok(tex)
        })?;
        Ok(Rc::new(GlTexture {
            ctx: ctx.clone(),
            tex,
            width,
            height,
        }))
    }
}

impl Drop for GlTexture {
    fn drop(&mut self) {
        unsafe {
            self.ctx.with_current(|| {
                glDeleteTextures(1, &self.tex);
                Ok(())
            });
        }
    }
}

pub fn with_scissor<T, F: FnOnce() -> T>(scissor: &Rect, f: F) -> T {
    return f();

    #[thread_local]
    static mut SCISSOR: *const Rect = ptr::null();

    unsafe {
        let prev = SCISSOR;
        if prev.is_null() {
            glEnable(GL_SCISSOR_TEST);
        }
        glScissor(
            scissor.x1(),
            scissor.x2(),
            scissor.width(),
            scissor.height(),
        );
        SCISSOR = scissor;
        let res = f();
        if prev.is_null() {
            glDisable(GL_SCISSOR_TEST);
        } else {
            let prev = prev.deref();
            glScissor(prev.x1(), prev.x2(), prev.width(), prev.height());
        }
        SCISSOR = prev;
        res
    }
}

pub struct GlShader {
    _ctx: Rc<EglContext>,
    shader: GLuint,
}

impl GlShader {
    pub unsafe fn compile(ctx: &Rc<EglContext>, ty: GLenum, src: &str) -> Result<Self, GlesError> {
        let shader = glCreateShader(ty);
        let res = GlShader {
            _ctx: ctx.clone(),
            shader,
        };
        let len = src.len() as _;
        glShaderSource(shader, 1, &(src.as_ptr() as _), &len);
        glCompileShader(shader);

        let mut ok = 0;
        glGetShaderiv(shader, GL_COMPILE_STATUS, &mut ok);
        if ok == GL_FALSE as _ {
            return Err(GlesError::ShaderCompileFailed);
        }
        Ok(res)
    }
}

impl Drop for GlShader {
    fn drop(&mut self) {
        unsafe {
            self._ctx.with_current(|| {
                glDeleteShader(self.shader);
                Ok(())
            });
        }
    }
}

pub struct GlProgram {
    _ctx: Rc<EglContext>,
    prog: GLuint,
}

impl GlProgram {
    pub unsafe fn link(vert: &GlShader, frag: &GlShader) -> Result<Self, GlesError> {
        let res = GlProgram {
            _ctx: vert._ctx.clone(),
            prog: glCreateProgram(),
        };
        glAttachShader(res.prog, vert.shader);
        glAttachShader(res.prog, frag.shader);
        glLinkProgram(res.prog);
        glDetachShader(res.prog, vert.shader);
        glDetachShader(res.prog, frag.shader);

        let mut ok = 0;
        glGetProgramiv(res.prog, GL_LINK_STATUS, &mut ok);
        if ok == GL_FALSE as _ {
            return Err(GlesError::ProgramLink);
        }

        Ok(res)
    }

    pub unsafe fn get_uniform_location(&self, name: &Ustr) -> GLint {
        glGetUniformLocation(self.prog, name.as_ptr() as _)
    }

    pub unsafe fn get_attrib_location(&self, name: &Ustr) -> GLint {
        glGetAttribLocation(self.prog, name.as_ptr() as _)
    }

    pub unsafe fn use_(&self) {
        glUseProgram(self.prog);
    }
}

impl Drop for GlProgram {
    fn drop(&mut self) {
        unsafe {
            self._ctx.with_current(|| {
                glDeleteProgram(self.prog);
                Ok(())
            });
        }
    }
}

pub struct GlRenderBuffer {
    pub img: Rc<EglImage>,
    pub ctx: Rc<EglContext>,
    rbo: GLuint,
}

impl GlRenderBuffer {
    pub fn from_image(
        img: &Rc<EglImage>,
        ctx: &Rc<EglContext>,
    ) -> Result<Rc<GlRenderBuffer>, GlesError> {
        ctx.with_current(|| unsafe {
            let mut rbo = 0;
            glGenRenderbuffers(1, &mut rbo);
            glBindRenderbuffer(GL_RENDERBUFFER, rbo);
            PROCS.glEGLImageTargetRenderbufferStorageOES(GL_RENDERBUFFER, GLeglImageOES(img.img.0));
            glBindRenderbuffer(GL_RENDERBUFFER, 0);
            Ok(Rc::new(GlRenderBuffer {
                img: img.clone(),
                ctx: ctx.clone(),
                rbo,
            }))
        })
    }

    pub fn create_framebuffer(self: &Rc<Self>) -> Result<Rc<GlFrameBuffer>, GlesError> {
        self.ctx.with_current(|| unsafe {
            let mut fbo = 0;
            glGenFramebuffers(1, &mut fbo);
            glBindFramebuffer(GL_FRAMEBUFFER, fbo);
            glFramebufferRenderbuffer(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_RENDERBUFFER,
                self.rbo,
            );
            let status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
            let fb = GlFrameBuffer {
                _rb: Some(self.clone()),
                _tex: None,
                ctx: self.ctx.clone(),
                fbo,
                width: self.img.width,
                height: self.img.height,
            };
            if status != GL_FRAMEBUFFER_COMPLETE {
                return Err(GlesError::CreateFramebuffer);
            }
            Ok(Rc::new(fb))
        })
    }
}

impl Drop for GlRenderBuffer {
    fn drop(&mut self) {
        self.ctx.with_current(|| {
            unsafe {
                glDeleteRenderbuffers(1, &self.rbo);
            }
            Ok(())
        });
    }
}

pub struct GlFrameBuffer {
    pub _rb: Option<Rc<GlRenderBuffer>>,
    pub _tex: Option<Rc<GlTexture>>,
    pub ctx: Rc<EglContext>,
    pub width: i32,
    pub height: i32,
    fbo: GLuint,
}

impl GlFrameBuffer {
    pub unsafe fn bind(&self) {
        glBindFramebuffer(GL_FRAMEBUFFER, self.fbo);
    }

    pub fn clear(&self, r: f32, g: f32, b: f32, a: f32) -> Result<(), GlesError> {
        self.ctx.with_current(|| unsafe {
            glBindFramebuffer(GL_FRAMEBUFFER, self.fbo);
            glClearColor(r, g, b, a);
            glClear(GL_COLOR_BUFFER_BIT);
            // glFlush();
            Ok(())
        })
    }
}

impl Drop for GlFrameBuffer {
    fn drop(&mut self) {
        self.ctx.with_current(|| {
            unsafe {
                glDeleteFramebuffers(1, &self.fbo);
            }
            Ok(())
        });
    }
}
