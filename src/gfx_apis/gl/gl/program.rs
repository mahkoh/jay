use {
    crate::gfx_apis::gl::{
        egl::context::EglContext,
        gl::{
            shader::GlShader,
            sys::{GLint, GLuint, GL_FALSE, GL_FRAGMENT_SHADER, GL_LINK_STATUS, GL_VERTEX_SHADER},
        },
        RenderError,
    },
    std::{ffi::CStr, rc::Rc},
};

pub struct GlProgram {
    pub ctx: Rc<EglContext>,
    pub prog: GLuint,
}

impl GlProgram {
    pub(in crate::gfx_apis::gl) unsafe fn from_shaders(
        ctx: &Rc<EglContext>,
        vert: &str,
        frag: &str,
    ) -> Result<Self, RenderError> {
        unsafe {
            let vert = GlShader::compile(ctx, GL_VERTEX_SHADER, vert)?;
            let frag = GlShader::compile(ctx, GL_FRAGMENT_SHADER, frag)?;
            Self::link(&vert, &frag)
        }
    }

    pub(in crate::gfx_apis::gl) unsafe fn link(
        vert: &GlShader,
        frag: &GlShader,
    ) -> Result<Self, RenderError> {
        unsafe {
            let gles = vert.ctx.dpy.gles;
            let res = GlProgram {
                ctx: vert.ctx.clone(),
                prog: (gles.glCreateProgram)(),
            };
            (gles.glAttachShader)(res.prog, vert.shader);
            (gles.glAttachShader)(res.prog, frag.shader);
            (gles.glLinkProgram)(res.prog);
            (gles.glDetachShader)(res.prog, vert.shader);
            (gles.glDetachShader)(res.prog, frag.shader);

            let mut ok = 0;
            (gles.glGetProgramiv)(res.prog, GL_LINK_STATUS, &mut ok);
            if ok == GL_FALSE as GLint {
                return Err(RenderError::ProgramLink);
            }

            Ok(res)
        }
    }

    pub unsafe fn get_uniform_location(&self, name: &CStr) -> GLint {
        unsafe { (self.ctx.dpy.gles.glGetUniformLocation)(self.prog, name.as_ptr() as _) }
    }

    pub unsafe fn get_attrib_location(&self, name: &CStr) -> GLint {
        unsafe { (self.ctx.dpy.gles.glGetAttribLocation)(self.prog, name.as_ptr() as _) }
    }
}

impl Drop for GlProgram {
    fn drop(&mut self) {
        unsafe {
            let _ = self.ctx.with_current(|| {
                (self.ctx.dpy.gles.glDeleteProgram)(self.prog);
                Ok(())
            });
        }
    }
}
