use {
    crate::{
        format::Format,
        render::{
            egl::{context::EglContext, image::EglImage, PROCS},
            gl::{
                frame_buffer::GlFrameBuffer,
                sys::{
                    glBindFramebuffer, glBindTexture, glCheckFramebufferStatus, glDeleteTextures,
                    glFramebufferTexture2D, glGenFramebuffers, glGenTextures, glPixelStorei,
                    glTexImage2D, glTexParameteri, GLint, GLuint, GL_CLAMP_TO_EDGE,
                    GL_COLOR_ATTACHMENT0, GL_FRAMEBUFFER, GL_FRAMEBUFFER_COMPLETE, GL_LINEAR,
                    GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_TEXTURE_MIN_FILTER, GL_TEXTURE_WRAP_S,
                    GL_TEXTURE_WRAP_T, GL_UNPACK_ROW_LENGTH_EXT,
                },
            },
            sys::GLeglImageOES,
            RenderError,
        },
    },
    std::{cell::Cell, ptr, rc::Rc},
};

pub struct GlTexture {
    pub(super) ctx: Rc<EglContext>,
    pub img: Option<Rc<EglImage>>,
    pub tex: GLuint,
    pub width: i32,
    pub height: i32,
}

impl GlTexture {
    #[allow(dead_code)]
    pub fn new(
        ctx: &Rc<EglContext>,
        format: &'static Format,
        width: i32,
        height: i32,
    ) -> Result<Rc<GlTexture>, RenderError> {
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
            img: None,
            tex,
            width,
            height,
        }))
    }

    #[allow(dead_code)]
    pub unsafe fn to_framebuffer(self: &Rc<Self>) -> Result<Rc<GlFrameBuffer>, RenderError> {
        self.ctx.with_current(|| {
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
                return Err(RenderError::CreateFramebuffer);
            }
            Ok(Rc::new(fb))
        })
    }

    pub fn import_img(ctx: &Rc<EglContext>, img: &Rc<EglImage>) -> Result<GlTexture, RenderError> {
        let tex = ctx.with_current(|| unsafe {
            let mut tex = 0;
            glGenTextures(1, &mut tex);
            glBindTexture(GL_TEXTURE_2D, tex);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            PROCS.glEGLImageTargetTexture2DOES(GL_TEXTURE_2D, GLeglImageOES(img.img.0));
            glBindTexture(GL_TEXTURE_2D, 0);
            Ok(tex)
        })?;
        Ok(GlTexture {
            ctx: ctx.clone(),
            img: Some(img.clone()),
            tex,
            width: img.width,
            height: img.height,
        })
    }

    pub fn import_shm(
        ctx: &Rc<EglContext>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> Result<GlTexture, RenderError> {
        if (stride * height) as usize > data.len() {
            return Err(RenderError::SmallImageBuffer);
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
        Ok(GlTexture {
            ctx: ctx.clone(),
            img: None,
            tex,
            width,
            height,
        })
    }
}

impl Drop for GlTexture {
    fn drop(&mut self) {
        let _ = self.ctx.with_current(|| unsafe {
            glDeleteTextures(1, &self.tex);
            Ok(())
        });
    }
}
