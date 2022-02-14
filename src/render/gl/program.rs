use crate::render::egl::context::EglContext;
use crate::render::gl::shader::GlShader;
use crate::render::gl::sys::{
    glAttachShader, glCreateProgram, glDeleteProgram, glDetachShader, glGetAttribLocation,
    glGetProgramiv, glGetUniformLocation, glLinkProgram, GLint, GLuint, GL_FALSE, GL_LINK_STATUS,
};
use crate::render::gl::sys::{GL_FRAGMENT_SHADER, GL_VERTEX_SHADER};
use crate::render::RenderError;
use std::rc::Rc;
use uapi::Ustr;

pub struct GlProgram {
    pub _ctx: Rc<EglContext>,
    pub prog: GLuint,
}

impl GlProgram {
    pub unsafe fn from_shaders(
        ctx: &Rc<EglContext>,
        vert: &str,
        frag: &str,
    ) -> Result<Self, RenderError> {
        let vert = GlShader::compile(ctx, GL_VERTEX_SHADER, vert)?;
        let frag = GlShader::compile(ctx, GL_FRAGMENT_SHADER, frag)?;
        Self::link(&vert, &frag)
    }

    pub unsafe fn link(vert: &GlShader, frag: &GlShader) -> Result<Self, RenderError> {
        let res = GlProgram {
            _ctx: vert.ctx.clone(),
            prog: glCreateProgram(),
        };
        glAttachShader(res.prog, vert.shader);
        glAttachShader(res.prog, frag.shader);
        glLinkProgram(res.prog);
        glDetachShader(res.prog, vert.shader);
        glDetachShader(res.prog, frag.shader);

        let mut ok = 0;
        glGetProgramiv(res.prog, GL_LINK_STATUS, &mut ok);
        if ok == GL_FALSE as GLint {
            return Err(RenderError::ProgramLink);
        }

        Ok(res)
    }

    pub unsafe fn get_uniform_location(&self, name: &Ustr) -> GLint {
        glGetUniformLocation(self.prog, name.as_ptr() as _)
    }

    pub unsafe fn get_attrib_location(&self, name: &Ustr) -> GLint {
        glGetAttribLocation(self.prog, name.as_ptr() as _)
    }
}

impl Drop for GlProgram {
    fn drop(&mut self) {
        unsafe {
            let _ = self._ctx.with_current(|| {
                glDeleteProgram(self.prog);
                Ok(())
            });
        }
    }
}
