use {
    crate::{
        ifs::wl_surface::{
            xdg_surface::{xdg_popup::XdgPopup, xdg_toplevel::XdgToplevel},
            xwindow::Xwindow,
            zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
            WlSurface,
        },
        tree::{ContainerNode, DisplayNode, FloatNode, Node, OutputNode, WorkspaceNode},
    },
    std::rc::Rc,
};

pub trait NodeVisitorBase: Sized {
    fn visit_surface(&mut self, node: &Rc<WlSurface>) {
        node.node_visit_children(self);
    }

    fn visit_container(&mut self, node: &Rc<ContainerNode>) {
        node.node_visit_children(self);
    }

    fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
        node.node_visit_children(self);
    }

    fn visit_popup(&mut self, node: &Rc<XdgPopup>) {
        node.node_visit_children(self);
    }

    fn visit_display(&mut self, node: &Rc<DisplayNode>) {
        node.node_visit_children(self);
    }

    fn visit_output(&mut self, node: &Rc<OutputNode>) {
        node.node_visit_children(self);
    }

    fn visit_float(&mut self, node: &Rc<FloatNode>) {
        node.node_visit_children(self);
    }

    fn visit_workspace(&mut self, node: &Rc<WorkspaceNode>) {
        node.node_visit_children(self);
    }

    fn visit_layer_surface(&mut self, node: &Rc<ZwlrLayerSurfaceV1>) {
        node.node_visit_children(self);
    }

    fn visit_xwindow(&mut self, node: &Rc<Xwindow>) {
        node.node_visit_children(self);
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
    fn visit_layer_surface(&mut self, node: &Rc<ZwlrLayerSurfaceV1>);
    fn visit_xwindow(&mut self, node: &Rc<Xwindow>);
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

    fn visit_layer_surface(&mut self, node: &Rc<ZwlrLayerSurfaceV1>) {
        <T as NodeVisitorBase>::visit_layer_surface(self, node)
    }

    fn visit_xwindow(&mut self, node: &Rc<Xwindow>) {
        <T as NodeVisitorBase>::visit_xwindow(self, node)
    }
}

pub struct GenericNodeVisitor<F> {
    f: F,
}

pub fn generic_node_visitor<F: FnMut(Rc<dyn Node>)>(f: F) -> GenericNodeVisitor<F> {
    GenericNodeVisitor { f }
}

impl<F: FnMut(Rc<dyn Node>)> NodeVisitor for GenericNodeVisitor<F> {
    fn visit_surface(&mut self, node: &Rc<WlSurface>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
    }

    fn visit_container(&mut self, node: &Rc<ContainerNode>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
    }

    fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
    }

    fn visit_popup(&mut self, node: &Rc<XdgPopup>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
    }

    fn visit_display(&mut self, node: &Rc<DisplayNode>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
    }

    fn visit_output(&mut self, node: &Rc<OutputNode>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
    }

    fn visit_float(&mut self, node: &Rc<FloatNode>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
    }

    fn visit_workspace(&mut self, node: &Rc<WorkspaceNode>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
    }

    fn visit_layer_surface(&mut self, node: &Rc<ZwlrLayerSurfaceV1>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
    }

    fn visit_xwindow(&mut self, node: &Rc<Xwindow>) {
        (self.f)(node.clone());
        node.node_visit_children(self);
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
