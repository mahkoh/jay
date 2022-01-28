use crate::egl::EglContext;
use crate::gles2::gl::{with_scissor, GlFrameBuffer, GlProgram, GlShader};
use crate::gles2::sys::{
    glActiveTexture, glBindTexture, glClear, glClearColor, glDisableVertexAttribArray,
    glDrawArrays, glEnableVertexAttribArray, glTexParameteri, glUniform1f, glUniform1i,
    glVertexAttribPointer, GLint, GL_COLOR_BUFFER_BIT, GL_FALSE, GL_FLOAT, GL_FRAGMENT_SHADER,
    GL_LINEAR, GL_TEXTURE0, GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_TRIANGLE_STRIP,
    GL_VERTEX_SHADER,
};
use crate::gles2::GlesError;
use crate::ifs::wl_buffer::WlBuffer;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::wl_surface::WlSurface;
use crate::pixman::Image;
use crate::render::{Border, Renderer};
use crate::servermem::ServerMem;
use crate::tree::{
    ContainerFocus, ContainerNode, ContainerSplit, CONTAINER_BORDER, CONTAINER_TITLE_HEIGHT,
};
use crate::tree::{FloatNode, OutputNode, WorkspaceNode};
use renderdoc::{RenderDoc, V100};
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::ops::Deref;
use std::ptr;
use std::rc::Rc;
use uapi::ustr;

pub const RENDERDOC: bool = false;

pub struct GlesRenderer {
    ctx: Rc<EglContext>,

    renderdoc: Option<RefCell<RenderDoc<V100>>>,

    tex_prog: GlProgram,
    tex_prog_pos: GLint,
    tex_prog_texcoord: GLint,
    tex_prog_tex: GLint,
}

impl GlesRenderer {
    pub unsafe fn new(ctx: &Rc<EglContext>) -> Result<Self, GlesError> {
        let vert = GlShader::compile(ctx, GL_VERTEX_SHADER, include_str!("shaders/tex.vert.glsl"))?;
        let frag = GlShader::compile(
            ctx,
            GL_FRAGMENT_SHADER,
            include_str!("shaders/tex.frag.glsl"),
        )?;
        let prog = GlProgram::link(&vert, &frag)?;
        Ok(Self {
            ctx: ctx.clone(),
            tex_prog_pos: prog.get_attrib_location(ustr!("pos")),
            tex_prog_texcoord: prog.get_attrib_location(ustr!("texcoord")),
            tex_prog_tex: prog.get_uniform_location(ustr!("tex")),
            tex_prog: prog,
            renderdoc: if RENDERDOC {
                Some(RefCell::new(RenderDoc::new().unwrap()))
            } else {
                None
            },
        })
    }

    pub fn render_fb<'a>(&'a self, fb: &'a GlFrameBuffer) -> GlesImageRenderer<'a> {
        if let Some(rd) = &self.renderdoc {
            rd.borrow_mut()
                .start_frame_capture(ptr::null(), ptr::null());
        }
        GlesImageRenderer {
            renderer: self,
            image: fb,
        }
    }
}

pub struct GlesImageRenderer<'a> {
    renderer: &'a GlesRenderer,
    image: &'a GlFrameBuffer,
}

impl Drop for GlesImageRenderer<'_> {
    fn drop(&mut self) {
        if let Some(rd) = &self.renderer.renderdoc {
            rd.borrow_mut().end_frame_capture(ptr::null(), ptr::null());
        }
    }
}

const NON_COLOR: (u8, u8, u8) = (100, 100, 100);
const CHILD_COLOR: (u8, u8, u8) = (200, 200, 200);
const YES_COLOR: (u8, u8, u8) = (0, 0, 255);

fn focus_color(focus: ContainerFocus) -> (u8, u8, u8) {
    match focus {
        ContainerFocus::None => NON_COLOR,
        ContainerFocus::Child => CHILD_COLOR,
        ContainerFocus::Yes => YES_COLOR,
    }
}

impl Renderer for GlesImageRenderer<'_> {
    fn render_output(&mut self, output: &OutputNode) {
        unsafe {
            glClearColor(0.0, 0.0, 0.0, 1.0);
            glClear(GL_COLOR_BUFFER_BIT);
        }
        if let Some(ws) = output.workspace.get() {
            self.render_workspace(&ws);
        }
    }

    fn render_workspace(&mut self, workspace: &WorkspaceNode) {
        if let Some(node) = workspace.container.get() {
            self.render_container(&node, 0, 0)
        }
    }

    fn render_container(&mut self, container: &ContainerNode, x: i32, y: i32) {
        let cwidth = container.width.get();
        let cheight = container.height.get();
        let num_children = container.num_children();
        if let Some(child) = container.mono_child.get() {
            let space_per_child = cwidth / num_children as i32;
            let mut rem = cwidth % num_children as i32;
            let mut pos = x;
            for child in container.children.iter() {
                let (r, g, b) = focus_color(child.focus.get());
                let mut width = space_per_child;
                if rem > 0 {
                    rem -= 1;
                    width += 1;
                }
                // let _ = self.image.fill_rect(
                //     r,
                //     g,
                //     b,
                //     255,
                //     pos,
                //     y,
                //     pos + width as i32,
                //     y + CONTAINER_TITLE_HEIGHT as i32,
                // );
                pos += width as i32;
            }
            with_scissor(&container.mono_body.get(), || {
                let content = container.mono_content.get();
                child.node.render(self, x + content.x1(), y + content.y1());
            });
        } else {
            let split = container.split.get();
            for (i, child) in container.children.iter().enumerate() {
                let body = child.body.get();
                if body.x1() >= cwidth || body.y1() >= cheight {
                    break;
                }
                let (r, g, b) = focus_color(child.focus.get());
                // let _ = self.image.fill_rect(
                //     r,
                //     g,
                //     b,
                //     255,
                //     x + body.x1(),
                //     y + body.y1() - CONTAINER_TITLE_HEIGHT,
                //     x + body.x2(),
                //     y + body.y1(),
                // );
                {
                    let mut x1 = x + body.x1();
                    let mut x2 = x + body.x2();
                    let mut y2 = y + body.y2();
                    let mut border = Border::empty();
                    if i < num_children {
                        if split == ContainerSplit::Horizontal {
                            border |= Border::RIGHT;
                            x2 += CONTAINER_BORDER;
                        } else if split == ContainerSplit::Vertical {
                            border |= Border::BOTTOM;
                            y2 += CONTAINER_BORDER;
                        }
                    }
                    if i > 0 && split == ContainerSplit::Horizontal {
                        border |= Border::LEFT;
                        x1 -= CONTAINER_BORDER;
                    }
                    // let _ = self.image.fill_inner_border(
                    //     r,
                    //     g,
                    //     b,
                    //     255,
                    //     x1,
                    //     y + body.y1() - CONTAINER_TITLE_HEIGHT,
                    //     x2,
                    //     y2,
                    //     CONTAINER_BORDER as i32,
                    //     border,
                    // );
                }
                with_scissor(&body, || {
                    let content = child.content.get();
                    child.node.render(self, x + content.x1(), y + content.y1());
                    // self.image.fill_inner_border(
                    //     0,
                    //     0,
                    //     255,
                    //     255,
                    //     x + body.x1(),
                    //     y + body.y1(),
                    //     x + body.x1() + body.width(),
                    //     y + body.y1() + body.height(),
                    //     2,
                    //     Border::all(),
                    // );
                    // self.image.fill_inner_border(
                    //     255,
                    //     0,
                    //     0,
                    //     255,
                    //     x + content.x1(),
                    //     y + content.y1(),
                    //     x + content.x1() + content.width(),
                    //     y + content.y1() + content.height(),
                    //     2,
                    //     Border::all(),
                    // );
                });
            }
        }
    }

    fn render_toplevel(&mut self, tl: &XdgToplevel, mut x: i32, mut y: i32) {
        let surface = &tl.xdg.surface;
        if let Some(geo) = tl.xdg.geometry() {
            let (xt, yt) = geo.translate(x, y);
            x = xt;
            y = yt;
        }
        self.render_surface(surface, x, y);
    }

    fn render_surface(&mut self, surface: &WlSurface, x: i32, y: i32) {
        let children = surface.children.borrow();
        let buffer = match surface.buffer.get() {
            Some(b) => b,
            _ => {
                log::warn!("surface has no buffer attached");
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
            render!(&children.above);
            self.render_buffer(&buffer, x, y);
            render!(&children.below);
        } else {
            self.render_buffer(&buffer, x, y);
        }
        let mut fr = surface.frame_requests.borrow_mut();
        for cb in fr.drain(..) {
            surface.client.dispatch_frame_requests.push(cb);
        }
    }

    fn render_buffer(&mut self, buffer: &WlBuffer, x: i32, y: i32) {
        let texture = match buffer.texture.get() {
            Some(t) => t,
            _ => return,
        };
        unsafe {
            glActiveTexture(GL_TEXTURE0);

            glBindTexture(GL_TEXTURE_2D, texture.tex);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);

            self.renderer.tex_prog.use_();

            glUniform1i(self.renderer.tex_prog_tex, 0);

            let texcoord: [f32; 8] = [1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0];

            let f_width = self.image.width as f32;
            let f_height = self.image.height as f32;

            let x1 = 2.0 * (x as f32 / f_width) - 1.0;
            let y1 = 2.0 * (y as f32 / f_height) - 1.0;
            let x2 = 2.0 * ((x + texture.width) as f32 / f_width) - 1.0;
            let y2 = 2.0 * ((y + texture.height) as f32 / f_height) - 1.0;

            let pos: [f32; 8] = [x2, y1, x1, y1, x2, y2, x1, y2];

            glVertexAttribPointer(
                self.renderer.tex_prog_texcoord as _,
                2,
                GL_FLOAT,
                GL_FALSE,
                0,
                texcoord.as_ptr() as _,
            );
            glVertexAttribPointer(
                self.renderer.tex_prog_pos as _,
                2,
                GL_FLOAT,
                GL_FALSE,
                0,
                pos.as_ptr() as _,
            );

            glEnableVertexAttribArray(self.renderer.tex_prog_texcoord as _);
            glEnableVertexAttribArray(self.renderer.tex_prog_pos as _);

            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);

            glDisableVertexAttribArray(self.renderer.tex_prog_texcoord as _);
            glDisableVertexAttribArray(self.renderer.tex_prog_pos as _);

            glBindTexture(GL_TEXTURE_2D, 0);
        }
    }

    fn render_floating(&mut self, floating: &FloatNode, x: i32, y: i32) {
        if let Some(child) = floating.child.get() {
            child.render(self, x, y)
        }
    }
}
