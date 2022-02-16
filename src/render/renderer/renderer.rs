use crate::format::Format;
use crate::ifs::wl_buffer::WlBuffer;
use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::ifs::wl_surface::WlSurface;
use crate::rect::Rect;
use crate::render::gl::frame_buffer::{with_scissor, GlFrameBuffer};
use crate::render::gl::sys::{
    glActiveTexture, glBindTexture, glDisableVertexAttribArray, glDrawArrays,
    glEnableVertexAttribArray, glTexParameteri, glUniform1i, glUniform4f, glUseProgram,
    glVertexAttribPointer, GL_FALSE, GL_FLOAT, GL_LINEAR, GL_TEXTURE0, GL_TEXTURE_2D,
    GL_TEXTURE_MIN_FILTER, GL_TRIANGLES, GL_TRIANGLE_STRIP,
};
use crate::render::renderer::context::RenderContext;
use crate::render::sys::{glDisable, glEnable, GL_BLEND};
use crate::render::Texture;
use crate::tree::{
    ContainerFocus, ContainerNode, ContainerSplit, FloatNode, OutputNode, WorkspaceNode,
    CONTAINER_BORDER, CONTAINER_TITLE_HEIGHT,
};
use std::ops::Deref;
use std::rc::Rc;
use std::slice;

const NON_COLOR: (f32, f32, f32) = (0.2, 0.2, 0.2);
const CHILD_COLOR: (f32, f32, f32) = (0.8, 0.8, 0.8);
const YES_COLOR: (f32, f32, f32) = (0.0, 0.0, 1.0);

fn focus_color(focus: ContainerFocus) -> (f32, f32, f32) {
    match focus {
        ContainerFocus::None => NON_COLOR,
        ContainerFocus::Child => CHILD_COLOR,
        ContainerFocus::Yes => YES_COLOR,
    }
}

const TITLE_COLOR: (f32, f32, f32) = ((0x46 as f32)/255., (0x04 as f32)/255., (0x17 as f32)/255.);
// const BORDER_COLOR: (f32, f32, f32) = ((0xba as f32)/255., (0x57 as f32)/255., (0x00 as f32)/255.);
const UNDERLINE_COLOR: (f32, f32, f32) = ((0x66 as f32)/255., (0x24 as f32)/255., (0x37 as f32)/255.);
const BORDER_COLOR: (f32, f32, f32) = ((0x36 as f32)/255., (0x00 as f32)/255., (0x07 as f32)/255.);

pub struct Renderer<'a> {
    pub(super) ctx: &'a RenderContext,
    pub(super) fb: &'a GlFrameBuffer,
}

impl Renderer<'_> {
    pub fn render_output(&mut self, output: &OutputNode, x: i32, y: i32) {
        if let Some(ws) = output.workspace.get() {
            self.render_workspace(&ws, x, y);
        }
    }

    pub fn render_workspace(&mut self, workspace: &WorkspaceNode, x: i32, y: i32) {
        if let Some(node) = workspace.container.get() {
            self.render_container(&node, x, y)
        }
        for stacked in workspace.stacked.iter() {
            let pos = stacked.absolute_position();
            stacked.render(self, pos.x1(), pos.y1());
        }
    }

    fn x_to_f(&self, x: i32) -> f32 {
        2.0 * (x as f32 / self.fb.width as f32) - 1.0
    }

    fn y_to_f(&self, y: i32) -> f32 {
        2.0 * (y as f32 / self.fb.height as f32) - 1.0
    }

    fn fill_boxes(&self, boxes: &[Rect], r: f32, g: f32, b: f32, a: f32) {
        let mut pos = Vec::with_capacity(boxes.len() * 12);
        for bx in boxes {
            let x1 = self.x_to_f(bx.x1());
            let y1 = self.y_to_f(bx.y1());
            let x2 = self.x_to_f(bx.x2());
            let y2 = self.y_to_f(bx.y2());
            pos.extend_from_slice(&[
                // triangle 1
                x2, y1, // top right
                x1, y1, // top left
                x1, y2, // bottom left
                // triangle 2
                x2, y1, // top right
                x1, y2, // bottom left
                x2, y2, // bottom right
            ]);
        }
        unsafe {
            glUseProgram(self.ctx.fill_prog.prog);
            glUniform4f(self.ctx.fill_prog_color, r, g, b, a);
            glVertexAttribPointer(
                self.ctx.fill_prog_pos as _,
                2,
                GL_FLOAT,
                GL_FALSE,
                0,
                pos.as_ptr() as _,
            );
            glEnableVertexAttribArray(self.ctx.fill_prog_pos as _);
            glDrawArrays(GL_TRIANGLES, 0, (boxes.len() * 6) as _);
            glDisableVertexAttribArray(self.ctx.fill_prog_pos as _);
        }
    }

    pub fn render_container(&mut self, container: &ContainerNode, x: i32, y: i32) {
        let cwidth = container.width.get();
        let cheight = container.height.get();
        let num_children = container.num_children();
        let title_rect =
            Rect::new_sized(x, y, container.width.get(), CONTAINER_TITLE_HEIGHT - 1).unwrap();
        let underline_rect =
            Rect::new_sized(x, y + CONTAINER_TITLE_HEIGHT - 1, container.width.get(), 1).unwrap();
        if let Some(child) = container.mono_child.get() {
            let space_per_child = cwidth / num_children as i32;
            let mut rem = cwidth % num_children as i32;
            let mut pos = x;
            let (r, g, b) = focus_color(ContainerFocus::None);
            self.fill_boxes(slice::from_ref(&title_rect), r, g, b, 1.0);
            let (r, g, b) = BORDER_COLOR;
            self.fill_boxes(slice::from_ref(&underline_rect), r, g, b, 1.0);
            for child in container.children.iter() {
                let focus = child.focus.get();
                let (r, g, b) = focus_color(focus);
                let mut width = space_per_child;
                if rem > 0 {
                    rem -= 1;
                    width += 1;
                }
                if focus != ContainerFocus::None {
                    let rect = Rect::new_sized(pos, y, width, CONTAINER_TITLE_HEIGHT).unwrap();
                    self.fill_boxes(slice::from_ref(&rect), r, g, b, 1.0);
                }
                pos += width as i32;
            }
            unsafe {
                with_scissor(&container.mono_body.get(), || {
                    let content = container.mono_content.get();
                    child.node.render(self, x + content.x1(), y + content.y1());
                });
            }
        } else {
            let split = container.split.get();
            let num_title_rects = if split == ContainerSplit::Horizontal {
                1
            } else {
                num_children
            };
            let mut title_rects = Vec::with_capacity(num_title_rects);
            let mut underline_rects = Vec::with_capacity(num_title_rects);
            let mut border_rects = Vec::with_capacity(num_children - 1);
            title_rects.push(title_rect);
            underline_rects.push(underline_rect);
            for (i, child) in container.children.iter().enumerate() {
                let body = child.body.get();
                if i + 1 < num_children {
                    let border_rect = if split == ContainerSplit::Horizontal {
                        Rect::new_sized(
                            x + body.x2(),
                            y + body.y1() - CONTAINER_TITLE_HEIGHT,
                            CONTAINER_BORDER,
                            container.height.get(),
                        )
                        .unwrap()
                    } else {
                        title_rects.push(Rect::new_sized(
                            x,
                            y + body.y2() + CONTAINER_BORDER,
                            container.width.get(),
                            CONTAINER_TITLE_HEIGHT - 1,
                        ).unwrap());
                        underline_rects.push(Rect::new_sized(
                            x,
                            y + body.y2() + CONTAINER_BORDER + CONTAINER_TITLE_HEIGHT - 1,
                            container.width.get(),
                            1,
                        ).unwrap());
                        Rect::new_sized(
                            x,
                            y + body.y2(),
                            container.width.get(),
                            CONTAINER_BORDER,
                        )
                        .unwrap()
                    };
                    border_rects.push(border_rect);
                }
            }
            {
                let (r, g, b) = TITLE_COLOR;
                self.fill_boxes(&title_rects, r, g, b, 1.0);
                let (r, g, b) = UNDERLINE_COLOR;
                self.fill_boxes(&underline_rects, r, g, b, 1.0);
                let (r, g, b) = BORDER_COLOR;
                self.fill_boxes(&border_rects, r, g, b, 1.0);
            }
            for child in container.children.iter() {
                let body = child.body.get();
                if body.x1() >= cwidth || body.y1() >= cheight {
                    break;
                }
                unsafe {
                    with_scissor(&body, || {
                        let content = child.content.get();
                        child.node.render(self, x + content.x1(), y + content.y1());
                    });
                }
            }
        }
    }

    pub fn render_xdg_surface(&mut self, xdg: &XdgSurface, mut x: i32, mut y: i32) {
        let surface = &xdg.surface;
        if let Some(geo) = xdg.geometry() {
            let (xt, yt) = geo.translate(x, y);
            x = xt;
            y = yt;
        }
        self.render_surface(surface, x, y);
    }

    pub fn render_surface(&mut self, surface: &WlSurface, x: i32, y: i32) {
        let children = surface.children.borrow();
        let buffer = match surface.buffer.get() {
            Some(b) => b,
            _ => {
                if !surface.is_cursor() {
                    log::warn!("surface has no buffer attached");
                }
                return;
            }
        };
        if let Some(children) = children.deref() {
            macro_rules! render {
                ($children:expr) => {
                    for child in $children.rev_iter() {
                        if child.pending.get() {
                            continue;
                        }
                        let pos = child.sub_surface.position.get();
                        self.render_surface(&child.sub_surface.surface, x + pos.x1(), y + pos.y1());
                    }
                };
            }
            render!(&children.below);
            self.render_buffer(&buffer, x, y);
            render!(&children.above);
        } else {
            self.render_buffer(&buffer, x, y);
        }
        let mut fr = surface.frame_requests.borrow_mut();
        for cb in fr.drain(..) {
            surface.client.dispatch_frame_requests.push(cb);
        }
    }

    pub fn render_buffer(&mut self, buffer: &WlBuffer, x: i32, y: i32) {
        if let Some(tex) = buffer.texture.get() {
            self.render_texture(&tex, x, y, buffer.format);
        }
    }

    pub fn render_texture(&mut self, texture: &Texture, x: i32, y: i32, format: &Format) {
        assert!(Rc::ptr_eq(&self.ctx.ctx, &texture.ctx.ctx));
        unsafe {
            glActiveTexture(GL_TEXTURE0);

            glBindTexture(GL_TEXTURE_2D, texture.gl.tex);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);

            let prog = match format.has_alpha {
                true => {
                    glEnable(GL_BLEND);
                    &self.ctx.tex_alpha_prog
                }
                false => {
                    glDisable(GL_BLEND);
                    &self.ctx.tex_prog
                }
            };

            glUseProgram(prog.prog.prog);

            glUniform1i(prog.tex, 0);

            let texcoord: [f32; 8] = [1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0];

            let f_width = self.fb.width as f32;
            let f_height = self.fb.height as f32;

            let x1 = 2.0 * (x as f32 / f_width) - 1.0;
            let y1 = 2.0 * (y as f32 / f_height) - 1.0;
            let x2 = 2.0 * ((x + texture.gl.width) as f32 / f_width) - 1.0;
            let y2 = 2.0 * ((y + texture.gl.height) as f32 / f_height) - 1.0;

            let pos: [f32; 8] = [
                x2, y1, // top right
                x1, y1, // top left
                x2, y2, // bottom right
                x1, y2, // bottom left
            ];

            glVertexAttribPointer(
                prog.texcoord as _,
                2,
                GL_FLOAT,
                GL_FALSE,
                0,
                texcoord.as_ptr() as _,
            );
            glVertexAttribPointer(prog.pos as _, 2, GL_FLOAT, GL_FALSE, 0, pos.as_ptr() as _);

            glEnableVertexAttribArray(prog.texcoord as _);
            glEnableVertexAttribArray(prog.pos as _);

            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);

            glDisableVertexAttribArray(prog.texcoord as _);
            glDisableVertexAttribArray(prog.pos as _);

            glBindTexture(GL_TEXTURE_2D, 0);
        }
    }

    pub fn render_floating(&mut self, floating: &FloatNode, x: i32, y: i32) {
        log::info!("rendering at {}x{}", x, y);
        if let Some(child) = floating.child.get() {
            child.render(self, x, y)
        }
    }
}
