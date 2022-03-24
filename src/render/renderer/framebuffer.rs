use crate::rect::Rect;
use crate::render::gl::frame_buffer::GlFrameBuffer;
use crate::render::gl::sys::{
    glBindFramebuffer, glClear, glClearColor, glViewport, GL_COLOR_BUFFER_BIT, GL_FRAMEBUFFER,
};
use crate::render::renderer::context::RenderContext;
use crate::render::renderer::renderer::Renderer;
use crate::render::sys::{glBlendFunc, GL_ONE, GL_ONE_MINUS_SRC_ALPHA, glFlush};
use crate::tree::Node;
use crate::State;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

pub struct Framebuffer {
    pub(super) ctx: Rc<RenderContext>,
    pub(super) gl: GlFrameBuffer,
}

impl Debug for Framebuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Framebuffer").finish_non_exhaustive()
    }
}

impl Framebuffer {
    pub fn clear(&self) {
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glClearColor(0.0, 0.0, 0.0, 0.0);
                glClear(GL_COLOR_BUFFER_BIT);
            }
            Ok(())
        });
    }

    pub fn render(&self, node: &dyn Node, state: &State, cursor_rect: Option<Rect>) {
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glClearColor(0.0, 0.0, 0.0, 1.0);
                glClear(GL_COLOR_BUFFER_BIT);
                glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            }
            let mut renderer = Renderer {
                ctx: &self.ctx,
                fb: &self.gl,
                state,
            };
            node.render(&mut renderer, 0, 0);
            if let Some(rect) = cursor_rect {
                let seats = state.globals.lock_seats();
                for seat in seats.values() {
                    if let Some(cursor) = seat.get_cursor() {
                        cursor.tick();
                        let extents = cursor.extents();
                        if let Some(dnd_icon) = seat.dnd_icon() {
                            let (x_hot, y_hot) = cursor.get_hotspot();
                            let extents = dnd_icon.extents.get().move_(
                                extents.x1() + x_hot + dnd_icon.buf_x.get(),
                                extents.y1() + y_hot + dnd_icon.buf_y.get(),
                            );
                            if extents.intersects(&rect) {
                                let (x, y) = rect.translate(extents.x1(), extents.y1());
                                renderer.render_surface(&dnd_icon, x, y);
                            }
                        }
                        if extents.intersects(&rect) {
                            let (x, y) = rect.translate(extents.x1(), extents.y1());
                            cursor.render(&mut renderer, x, y);
                        }
                    }
                }
            }
            unsafe {
                glFlush();
            }
            Ok(())
        });
    }
}
