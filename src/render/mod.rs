use crate::ifs::wl_buffer::WlBuffer;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::wl_surface::WlSurface;
use crate::tree::ContainerNode;
use crate::tree::{FloatNode, OutputNode, WorkspaceNode};

pub mod pixman;

bitflags::bitflags! {
    pub struct Border: u32 {
        const NONE    = 0b0000;
        const LEFT    = 0b0001;
        const TOP     = 0b0010;
        const RIGHT   = 0b0100;
        const BOTTOM  = 0b1000;
        const ALL     = 0b1111;
    }
}

pub trait Renderer {
    fn render_output(&mut self, output: &OutputNode);
    fn render_workspace(&mut self, workspace: &WorkspaceNode);
    fn render_container(&mut self, container: &ContainerNode, x: i32, y: i32);
    fn render_toplevel(&mut self, toplevel: &XdgToplevel, x: i32, y: i32);
    fn render_surface(&mut self, surface: &WlSurface, x: i32, y: i32);
    fn render_buffer(&mut self, buffer: &WlBuffer, x: i32, y: i32);
    fn render_floating(&mut self, floating: &FloatNode, x: i32, y: i32);
}
