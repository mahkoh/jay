use crate::render::gl::frame_buffer::GlFrameBuffer;
use crate::render::gl::sys::{
    glBindFramebuffer, glClear, glClearColor, glViewport, GL_COLOR_BUFFER_BIT, GL_FRAMEBUFFER,
};
use crate::render::renderer::context::RenderContext;
use crate::render::renderer::renderer::Renderer;
use crate::tree::Node;
use std::ptr;
use std::rc::Rc;

pub struct Framebuffer {
    pub(super) ctx: Rc<RenderContext>,
    pub(super) gl: GlFrameBuffer,
}

impl Framebuffer {
    pub fn render(&self, node: &dyn Node) {
        let _ = self.ctx.ctx.with_current(|| {
            if let Some(rd) = &self.ctx.renderdoc {
                rd.borrow_mut()
                    .start_frame_capture(ptr::null(), ptr::null());
            }
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glClearColor(0.0, 0.0, 0.0, 1.0);
                glClear(GL_COLOR_BUFFER_BIT);
            }
            let mut renderer = Renderer {
                ctx: &self.ctx,
                fb: &self.gl,
            };
            node.render(&mut renderer, 0, 0);
            if let Some(rd) = &self.ctx.renderdoc {
                rd.borrow_mut().end_frame_capture(ptr::null(), ptr::null());
            }
            Ok(())
        });
    }
}
