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
use std::ops::Deref;
use std::rc::Rc;

pub struct PixmanRenderer<'a> {
    image: &'a Image<Rc<ServerMem>>,
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

impl<'a> PixmanRenderer<'a> {
    pub fn new(image: &'a Image<Rc<ServerMem>>) -> Self {
        Self { image }
    }
}

impl Renderer for PixmanRenderer<'_> {
    fn render_output(&mut self, output: &OutputNode) {
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
                let _ = self.image.fill_rect(
                    r,
                    g,
                    b,
                    255,
                    pos,
                    y,
                    pos + width as i32,
                    y + CONTAINER_TITLE_HEIGHT as i32,
                );
                pos += width as i32;
            }
            self.image.with_clip(container.mono_body.get(), || {
                let content = container.mono_content.get();
                child
                    .node
                    .render(self, x + content.x1(), y + content.y1());
            });
        } else {
            let split = container.split.get();
            for (i, child) in container.children.iter().enumerate() {
                let body = child.body.get();
                if body.x1() >= cwidth || body.y1() >= cheight {
                    break;
                }
                let (r, g, b) = focus_color(child.focus.get());
                let _ = self.image.fill_rect(
                    r,
                    g,
                    b,
                    255,
                    x + body.x1(),
                    y + body.y1() - CONTAINER_TITLE_HEIGHT,
                    x + body.x2(),
                    y + body.y1(),
                );
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
                    let _ = self.image.fill_inner_border(
                        r,
                        g,
                        b,
                        255,
                        x1,
                        y + body.y1() - CONTAINER_TITLE_HEIGHT,
                        x2,
                        y2,
                        CONTAINER_BORDER as i32,
                        border,
                    );
                }
                self.image.with_clip(body, || {
                    let content = child.content.get();
                    child.node.render(self, x + content.x1(), y + content.y1());
                    self.image.fill_inner_border(0, 0, 255, 255, x + body.x1(), y + body.y1(), x + body.x1() + body.width(), y + body.y1() + body.height(), 2, Border::all());
                    self.image.fill_inner_border(255, 0, 0, 255, x + content.x1(), y + content.y1(), x + content.x1() + content.width(), y + content.y1() + content.height(), 2, Border::all());
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
                return
            },
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
        if let Err(e) = self.image.add_image(&buffer.image, x, y) {
            let client = &buffer.client;
            log::error!("Could not access client {} memory: {:#}", client.id, e);
            if let Ok(d) = client.display() {
                client.fatal_event(
                    d.implementation_error(format!("Could not access memory: {:#}", e)),
                );
            } else {
                client.state.clients.kill(client.id);
            }
        }
    }

    fn render_floating(&mut self, floating: &FloatNode, x: i32, y: i32) {
        if let Some(child) = floating.child.get() {
            child.render(self, x, y)
        }
    }
}
