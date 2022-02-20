use crate::ifs::wl_surface::xdg_surface::xdg_popup::XdgPopup;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::wl_surface::WlSurface;
use crate::tree::{ContainerNode, FloatNode, Node, OutputNode, WorkspaceNode};
use crate::DisplayNode;
use std::rc::Rc;

pub trait NodeVisitorBase: Sized {
    fn visit_surface(&mut self, node: &Rc<WlSurface>) {
        node.visit_children(self);
    }

    fn visit_container(&mut self, node: &Rc<ContainerNode>) {
        node.visit_children(self);
    }

    fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
        node.visit_children(self);
    }

    fn visit_popup(&mut self, node: &Rc<XdgPopup>) {
        node.visit_children(self);
    }

    fn visit_display(&mut self, node: &Rc<DisplayNode>) {
        node.visit_children(self);
    }

    fn visit_output(&mut self, node: &Rc<OutputNode>) {
        node.visit_children(self);
    }

    fn visit_float(&mut self, node: &Rc<FloatNode>) {
        node.visit_children(self);
    }

    fn visit_workspace(&mut self, node: &Rc<WorkspaceNode>) {
        node.visit_children(self);
    }
}

pub trait NodeVisitor {
    fn visit_surface(&mut self, node: &Rc<WlSurface>);
    fn visit_container(&mut self, node: &Rc<ContainerNode>);
    fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>);
    fn visit_popup(&mut self, node: &Rc<XdgPopup>);
    fn visit_display(&mut self, node: &Rc<DisplayNode>);
    fn visit_output(&mut self, node: &Rc<OutputNode>);
    fn visit_float(&mut self, node: &Rc<FloatNode>);
    fn visit_workspace(&mut self, node: &Rc<WorkspaceNode>);
}

impl<T: NodeVisitorBase> NodeVisitor for T {
    fn visit_surface(&mut self, node: &Rc<WlSurface>) {
        <T as NodeVisitorBase>::visit_surface(self, node)
    }

    fn visit_container(&mut self, node: &Rc<ContainerNode>) {
        <T as NodeVisitorBase>::visit_container(self, node)
    }

    fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
        <T as NodeVisitorBase>::visit_toplevel(self, node)
    }

    fn visit_popup(&mut self, node: &Rc<XdgPopup>) {
        <T as NodeVisitorBase>::visit_popup(self, node)
    }

    fn visit_display(&mut self, node: &Rc<DisplayNode>) {
        <T as NodeVisitorBase>::visit_display(self, node)
    }

    fn visit_output(&mut self, node: &Rc<OutputNode>) {
        <T as NodeVisitorBase>::visit_output(self, node)
    }

    fn visit_float(&mut self, node: &Rc<FloatNode>) {
        <T as NodeVisitorBase>::visit_float(self, node)
    }

    fn visit_workspace(&mut self, node: &Rc<WorkspaceNode>) {
        <T as NodeVisitorBase>::visit_workspace(self, node)
    }
}

// pub fn visit_containers<F: FnMut(&Rc<ContainerNode>)>(f: F) -> impl NodeVisitor {
//     struct V<F>(F);
//     impl<F: FnMut(&Rc<ContainerNode>)> NodeVisitorBase for V<F> {
//         fn visit_container(&mut self, node: &Rc<ContainerNode>) {
//             (self.0)(node);
//             node.visit_children(self);
//         }
//     }
//     V(f)
// }
//
// pub fn visit_floats<F: FnMut(&Rc<FloatNode>)>(f: F) -> impl NodeVisitor {
//     struct V<F>(F);
//     impl<F: FnMut(&Rc<FloatNode>)> NodeVisitorBase for V<F> {
//         fn visit_float(&mut self, node: &Rc<FloatNode>) {
//             (self.0)(node);
//             node.visit_children(self);
//         }
//     }
//     V(f)
// }
