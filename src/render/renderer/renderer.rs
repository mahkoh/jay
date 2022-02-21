use crate::format::{Format, ARGB8888};
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
use crate::theme::Color;
use crate::tree::{
    ContainerFocus, ContainerNode, ContainerSplit, FloatNode, OutputNode, WorkspaceNode,
};
use crate::State;
use std::ops::Deref;
use std::rc::Rc;
use std::slice;
use crate::ifs::wl_surface::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

const NON_COLOR: Color = Color::from_rgbaf(0.2, 0.2, 0.2, 1.0);
const CHILD_COLOR: Color = Color::from_rgbaf(0.8, 0.8, 0.8, 1.0);
const YES_COLOR: Color = Color::from_rgbaf(0.0, 0.0, 1.0, 1.0);

fn focus_color(focus: ContainerFocus) -> Color {
    match focus {
        ContainerFocus::None => NON_COLOR,
        ContainerFocus::Child => CHILD_COLOR,
        ContainerFocus::Yes => YES_COLOR,
    }
}

pub struct Renderer<'a> {
    pub(super) ctx: &'a Rc<RenderContext>,
    pub(super) fb: &'a GlFrameBuffer,
    pub(super) state: &'a State,
}

impl Renderer<'_> {
    pub fn render_output(&mut self, output: &OutputNode, x: i32, y: i32) {
        macro_rules! render_layer {
            ($layer:expr) => {
                for ls in $layer.iter() {
                    let pos = ls.position();
                    self.render_layer_surface(ls.deref(), pos.x1(), pos.y1());
                }
            }
        }
        render_layer!(output.layers[0]);
        render_layer!(output.layers[1]);
        if let Some(ws) = output.workspace.get() {
            self.render_workspace(&ws, x, y);
        }
        render_layer!(output.layers[2]);
        render_layer!(output.layers[3]);
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

    fn fill_boxes(&self, boxes: &[Rect], color: &Color) {
        if boxes.is_empty() {
            return;
        }
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
            glUniform4f(self.ctx.fill_prog_color, color.r, color.g, color.b, color.a);
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
        let border_width = self.state.theme.border_width.get();
        let title_height = self.state.theme.title_height.get();
        let cwidth = container.width.get();
        let cheight = container.height.get();
        let num_children = container.num_children();
        let title_rect = Rect::new_sized(x, y, container.width.get(), title_height).unwrap();
        let underline_rect =
            Rect::new_sized(x, y + title_height, container.width.get(), 1).unwrap();
        let mut titles = vec![];
        if let Some(child) = container.mono_child.get() {
            let space_per_child = cwidth / num_children as i32;
            let mut rem = cwidth % num_children as i32;
            let mut pos = x;
            let color = focus_color(ContainerFocus::None);
            self.fill_boxes(slice::from_ref(&title_rect), &color);
            let c = self.state.theme.border_color.get();
            self.fill_boxes(slice::from_ref(&underline_rect), &c);
            for child in container.children.iter() {
                let focus = child.focus.get();
                let color = focus_color(focus);
                let mut width = space_per_child;
                if rem > 0 {
                    rem -= 1;
                    width += 1;
                }
                if let Some(title) = child.title_texture.get() {
                    titles.push((pos, 0, title));
                }
                if focus != ContainerFocus::None {
                    let rect = Rect::new_sized(pos, y, width, title_height).unwrap();
                    self.fill_boxes(slice::from_ref(&rect), &color);
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
            let mut active_rects = Vec::new();
            title_rects.push(title_rect);
            underline_rects.push(underline_rect);
            for (i, child) in container.children.iter().enumerate() {
                let body = child.body.get();
                if let Some(title) = child.title_texture.get() {
                    titles.push((body.x1(), body.y1() - title_height - 1, title));
                }
                if child.active.get() {
                    active_rects.push(
                        Rect::new_sized(
                            x + body.x1(),
                            y + body.y1() - title_height - 1,
                            body.width(),
                            title_height,
                        )
                        .unwrap(),
                    );
                }
                if i + 1 < num_children {
                    let border_rect = if split == ContainerSplit::Horizontal {
                        Rect::new_sized(
                            x + body.x2(),
                            y + body.y1() - title_height - 1,
                            border_width,
                            container.height.get(),
                        )
                        .unwrap()
                    } else {
                        title_rects.push(
                            Rect::new_sized(
                                x,
                                y + body.y2() + border_width,
                                container.width.get(),
                                title_height,
                            )
                            .unwrap(),
                        );
                        underline_rects.push(
                            Rect::new_sized(
                                x,
                                y + body.y2() + border_width + title_height,
                                container.width.get(),
                                1,
                            )
                            .unwrap(),
                        );
                        Rect::new_sized(x, y + body.y2(), container.width.get(), border_width)
                            .unwrap()
                    };
                    border_rects.push(border_rect);
                }
            }
            {
                let c = self.state.theme.title_color.get();
                self.fill_boxes(&title_rects, &c);
                let c = self.state.theme.active_title_color.get();
                self.fill_boxes(&active_rects, &c);
                let c = self.state.theme.underline_color.get();
                self.fill_boxes(&underline_rects, &c);
                let c = self.state.theme.border_color.get();
                self.fill_boxes(&border_rects, &c);
                for (tx, ty, tex) in titles {
                    self.render_texture(&tex, x + tx, y + ty, ARGB8888);
                }
            }
            for child in container.children.iter() {
                let body = child.body.get();
                if body.x1() >= cwidth || body.y1() >= cheight {
                    break;
                }
                let body = body.move_(container.abs_x1.get(), container.abs_y1.get());
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
        let child = match floating.child.get() {
            Some(c) => c,
            _ => return,
        };
        let pos = floating.position.get();
        let theme = &self.state.theme;
        let th = theme.title_height.get();
        let bw = theme.border_width.get();
        let bc = theme.border_color.get();
        let tc = match floating.active.get() {
            true => theme.active_title_color.get(),
            false => theme.title_color.get(),
        };
        let uc = theme.underline_color.get();
        let borders = [
            Rect::new_sized(x, y, pos.width(), bw).unwrap(),
            Rect::new_sized(x, y + bw, bw, pos.height() - bw).unwrap(),
            Rect::new_sized(x + pos.width() - bw, y + bw, bw, pos.height() - bw).unwrap(),
            Rect::new_sized(x + bw, y + pos.height() - bw, pos.width() - 2 * bw, bw).unwrap(),
        ];
        self.fill_boxes(&borders, &bc);
        let title = [Rect::new_sized(x + bw, y + bw, pos.width() - 2 * bw, th).unwrap()];
        self.fill_boxes(&title, &tc);
        let title_underline =
            [Rect::new_sized(x + bw, y + bw + th, pos.width() - 2 * bw, 1).unwrap()];
        self.fill_boxes(&title_underline, &uc);
        if let Some(title) = floating.title_texture.get() {
            self.render_texture(&title, x + bw, y + bw, ARGB8888);
        }
        let body = Rect::new_sized(
            x + bw,
            y + bw + th + 1,
            pos.width() - 2 * bw,
            pos.height() - 2 * bw - th - 1,
        )
        .unwrap();
        unsafe {
            with_scissor(&body, || {
                child.render(self, body.x1(), body.y1());
            });
        }
    }

    pub fn render_layer_surface(&mut self, surface: &ZwlrLayerSurfaceV1, x: i32, y: i32) {
        unsafe {
            let body = surface.position();
            with_scissor(&body, || {
                self.render_surface(&surface.surface, x, y);
            });
        }
    }
}
