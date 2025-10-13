use {
    crate::{
        client::{Client, ClientError},
        globals::GlobalBase,
        ifs::{
            wl_surface::{
                ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
                x_surface::xwindow::Xwindow,
                xdg_surface::{xdg_popup::XdgPopup, xdg_toplevel::XdgToplevel},
                zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
            },
            wp_content_type_v1,
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        tree::{
            self, ContainerNode, DisplayNode, FloatNode, Node, NodeVisitor, OutputNode,
            PlaceholderNode, ToplevelData, ToplevelNodeBase, ToplevelType, WorkspaceNode,
        },
        utils::{opaque::OpaqueError, opt::Opt, toplevel_identifier::ToplevelIdentifier},
        wire::{JayTreeQueryId, jay_tree_query::*},
    },
    isnt::std_1::primitive::IsntStrExt,
    std::{
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
        str::FromStr,
    },
    thiserror::Error,
};

pub const TREE_TY_DISPLAY: u32 = 1;
pub const TREE_TY_OUTPUT: u32 = 2;
pub const TREE_TY_WORKSPACE: u32 = 3;
pub const TREE_TY_FLOAT: u32 = 4;
pub const TREE_TY_CONTAINER: u32 = 5;
pub const TREE_TY_PLACEHOLDER: u32 = 6;
pub const TREE_TY_XDG_TOPLEVEL: u32 = 7;
pub const TREE_TY_X_WINDOW: u32 = 8;
pub const TREE_TY_XDG_POPUP: u32 = 9;
pub const TREE_TY_LAYER_SURFACE: u32 = 10;
pub const TREE_TY_LOCK_SURFACE: u32 = 11;

const CONTENT_TYPE_SINCE: Version = Version(20);

pub struct JayTreeQuery {
    pub id: JayTreeQueryId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    recursive: Cell<bool>,
    root: RefCell<Option<Root>>,
}

enum Root {
    Display,
    WorkspaceNode(Rc<Opt<WorkspaceNode>>),
    WorkspaceName(String),
    ToplevelId(ToplevelIdentifier),
}

impl JayTreeQuery {
    pub fn new(client: &Rc<Client>, id: JayTreeQueryId, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            recursive: Cell::new(false),
            root: Default::default(),
        }
    }

    fn send_node_position(&self, node: &dyn Node) {
        let rect = node.node_mapped_position();
        self.send_position(rect);
    }

    fn send_position(&self, rect: Rect) {
        self.client.event(Position {
            self_id: self.id,
            x: rect.x1(),
            y: rect.y1(),
            w: rect.width(),
            h: rect.height(),
        });
    }

    fn send_not_found(&self) {
        self.client.event(NotFound { self_id: self.id });
    }

    fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    fn send_end(&self) {
        self.client.event(End { self_id: self.id });
    }

    fn send_start(&self, ty: u32) {
        self.client.event(Start {
            self_id: self.id,
            ty,
        });
    }

    fn send_client(&self, node: &impl Node) {
        if let Some(id) = node.node_client_id() {
            self.client.event(ClientId {
                self_id: self.id,
                id: id.raw(),
            });
        }
    }

    fn send_workspace_name(&self, name: &str) {
        self.client.event(WorkspaceName {
            self_id: self.id,
            name,
        });
    }

    fn send_output_name(&self, name: &str) {
        self.client.event(OutputName {
            self_id: self.id,
            name,
        });
    }

    fn send_toplevel(&self, data: &ToplevelData) {
        self.client.event(Start {
            self_id: self.id,
            ty: match &data.kind {
                ToplevelType::Container => TREE_TY_CONTAINER,
                ToplevelType::Placeholder(_) => TREE_TY_PLACEHOLDER,
                ToplevelType::XdgToplevel(_) => TREE_TY_XDG_TOPLEVEL,
                ToplevelType::XWindow(_) => TREE_TY_X_WINDOW,
            },
        });
        self.client.event(ToplevelId {
            self_id: self.id,
            id: &data.identifier.get().to_string(),
        });
        self.send_position(data.desired_extents.get());
        if let Some(cl) = data.client.as_ref().map(|c| c.id.raw()) {
            self.client.event(ClientId {
                self_id: self.id,
                id: cl,
            });
        }
        self.client.event(Title {
            self_id: self.id,
            title: &data.title.borrow(),
        });
        if let Some(w) = data.workspace.get() {
            self.send_workspace_name(&w.name);
        }
        match &data.kind {
            ToplevelType::Container => {}
            ToplevelType::Placeholder(id) => {
                if let Some(id) = *id {
                    self.client.event(PlaceholderFor {
                        self_id: self.id,
                        id: &id.to_string(),
                    });
                }
            }
            ToplevelType::XdgToplevel(d) => {
                self.client.event(AppId {
                    self_id: self.id,
                    app_id: &data.app_id.borrow(),
                });
                let tag = &*d.tag.borrow();
                if tag.is_not_empty() {
                    self.client.event(Tag {
                        self_id: self.id,
                        tag,
                    });
                }
            }
            ToplevelType::XWindow(d) => {
                if let Some(class) = &*d.info.class.borrow() {
                    self.client.event(XClass {
                        self_id: self.id,
                        class,
                    });
                }
                if let Some(instance) = &*d.info.instance.borrow() {
                    self.client.event(XInstance {
                        self_id: self.id,
                        instance,
                    });
                }
                if let Some(role) = &*d.info.role.borrow() {
                    self.client.event(XRole {
                        self_id: self.id,
                        role,
                    });
                }
            }
        }
        if data.parent_is_float.get() {
            self.client.event(Floating { self_id: self.id });
        }
        if data.visible.get() {
            self.client.event(Visible { self_id: self.id });
        }
        if data.wants_attention.get() {
            self.client.event(Urgent { self_id: self.id });
        }
        for seat_id in data.seat_foci.lock().keys() {
            for seat in data.state.globals.seats.lock().values() {
                if seat.id() == *seat_id {
                    self.client.event(Focused {
                        self_id: self.id,
                        global: seat.name().raw(),
                    });
                }
            }
        }
        if data.is_fullscreen.get() {
            self.client.event(Fullscreen { self_id: self.id });
        }
        if let Some(ws) = data.workspace.get() {
            self.client.event(Workspace {
                self_id: self.id,
                name: &ws.name,
            });
        }
        if self.version >= CONTENT_TYPE_SINCE
            && let Some(ct) = data.content_type.get()
        {
            use wp_content_type_v1::ContentType::*;
            self.client.event(ContentType {
                self_id: self.id,
                ty: match ct {
                    Photo => "photo",
                    Video => "video",
                    Game => "game",
                },
            });
        }
    }
}

impl JayTreeQueryRequestHandler for JayTreeQuery {
    type Error = JayTreeQueryError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn execute(&self, _req: Execute, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(root) = &*self.root.borrow() else {
            return Err(JayTreeQueryError::NoRootSet);
        };
        match root {
            Root::Display => Visitor(self).visit_display(&self.client.state.root),
            Root::WorkspaceNode(n) => match n.get() {
                Some(n) => Visitor(self).visit_workspace(&n),
                None => self.send_not_found(),
            },
            Root::WorkspaceName(n) => match self.client.state.workspaces.get(n) {
                Some(n) => Visitor(self).visit_workspace(&n),
                None => self.send_not_found(),
            },
            Root::ToplevelId(id) => match self
                .client
                .state
                .toplevels
                .get(id)
                .and_then(|t| t.upgrade())
            {
                Some(t) => t.node_visit(&mut Visitor(self)),
                None => self.send_not_found(),
            },
        }
        self.send_done();
        Ok(())
    }

    fn set_root_display(&self, _req: SetRootDisplay, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.root.borrow_mut() = Some(Root::Display);
        Ok(())
    }

    fn set_recursive(&self, req: SetRecursive, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.recursive.set(req.recursive != 0);
        Ok(())
    }

    fn set_root_workspace(
        &self,
        req: SetRootWorkspace,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let ws = self.client.lookup(req.workspace)?;
        let opt = match ws.workspace.get() {
            Some(ws) => ws.opt.clone(),
            _ => Default::default(),
        };
        let root = &mut *self.root.borrow_mut();
        *root = Some(Root::WorkspaceNode(opt));
        Ok(())
    }

    fn set_root_workspace_name(
        &self,
        req: SetRootWorkspaceName,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let root = &mut *self.root.borrow_mut();
        *root = Some(Root::WorkspaceName(req.workspace.to_owned()));
        Ok(())
    }

    fn set_root_toplevel(&self, req: SetRootToplevel, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tl = self.client.lookup(req.toplevel)?;
        let root = &mut *self.root.borrow_mut();
        *root = Some(Root::ToplevelId(tl.toplevel.tl_data().identifier.get()));
        Ok(())
    }

    fn set_root_window_id(
        &self,
        req: SetRootWindowId<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let id =
            ToplevelIdentifier::from_str(req.id).map_err(JayTreeQueryError::InvalidToplevelId)?;
        let root = &mut *self.root.borrow_mut();
        *root = Some(Root::ToplevelId(id));
        Ok(())
    }
}

struct Visitor<'a>(&'a JayTreeQuery);

impl tree::NodeVisitorBase for Visitor<'_> {
    fn visit_container(&mut self, node: &Rc<ContainerNode>) {
        let s = self.0;
        s.send_toplevel(node.tl_data());
        if s.recursive.get() {
            node.node_visit_children(self);
        }
        s.send_end();
    }

    fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
        let s = self.0;
        s.send_toplevel(node.tl_data());
        node.xdg.for_each_popup(|popup| {
            NodeVisitor::visit_popup(self, popup);
        });
        s.send_end();
    }

    fn visit_popup(&mut self, node: &Rc<XdgPopup>) {
        let s = self.0;
        s.send_start(TREE_TY_XDG_POPUP);
        s.send_node_position(&**node);
        s.send_end();
    }

    fn visit_display(&mut self, node: &Rc<DisplayNode>) {
        let s = self.0;
        s.send_start(TREE_TY_DISPLAY);
        s.send_node_position(&**node);
        if s.recursive.get() {
            for output in node.outputs.lock().values() {
                NodeVisitor::visit_output(self, output);
            }
            for stacked in node.stacked.iter() {
                if stacked.stacked_has_workspace_link() {
                    continue;
                }
                stacked.deref().clone().node_visit(self);
            }
        }
        s.send_end();
    }

    fn visit_output(&mut self, node: &Rc<OutputNode>) {
        let s = self.0;
        s.send_start(TREE_TY_OUTPUT);
        s.send_node_position(&**node);
        s.send_output_name(&node.global.connector.name);
        if s.recursive.get() {
            node.node_visit_children(self);
        }
        s.send_end();
    }

    fn visit_float(&mut self, node: &Rc<FloatNode>) {
        let s = self.0;
        s.send_start(TREE_TY_FLOAT);
        s.send_node_position(&**node);
        if s.recursive.get() {
            node.node_visit_children(self);
        }
        s.send_end();
    }

    fn visit_workspace(&mut self, node: &Rc<WorkspaceNode>) {
        let s = self.0;
        s.send_start(TREE_TY_WORKSPACE);
        s.send_node_position(&**node);
        s.send_workspace_name(&node.name);
        s.send_output_name(&node.current.output.get().global.connector.name);
        for stacked in node.stacked.iter() {
            if stacked.stacked_is_xdg_popup() {
                continue;
            }
            stacked.deref().clone().node_visit(self);
        }
        if s.recursive.get() {
            node.node_visit_children(self);
        }
        s.send_end();
    }

    fn visit_layer_surface(&mut self, node: &Rc<ZwlrLayerSurfaceV1>) {
        let s = self.0;
        s.send_start(TREE_TY_LAYER_SURFACE);
        s.send_client(&**node);
        s.send_node_position(&**node);
        node.for_each_popup(|popup| {
            NodeVisitor::visit_popup(self, popup);
        });
        s.send_end();
    }

    fn visit_xwindow(&mut self, node: &Rc<Xwindow>) {
        let s = self.0;
        s.send_toplevel(node.tl_data());
        s.send_end();
    }

    fn visit_placeholder(&mut self, node: &Rc<PlaceholderNode>) {
        let s = self.0;
        s.send_toplevel(node.tl_data());
        s.send_end();
    }

    fn visit_lock_surface(&mut self, node: &Rc<ExtSessionLockSurfaceV1>) {
        let s = self.0;
        s.send_start(TREE_TY_LOCK_SURFACE);
        s.send_client(&**node);
        s.send_node_position(&**node);
        s.send_end();
    }
}

object_base! {
    self = JayTreeQuery;
    version = self.version;
}

impl Object for JayTreeQuery {}

simple_add_obj!(JayTreeQuery);

#[derive(Debug, Error)]
pub enum JayTreeQueryError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Toplevel id is ill-formed")]
    InvalidToplevelId(OpaqueError),
    #[error("No root node was set")]
    NoRootSet,
}
efrom!(JayTreeQueryError, ClientError);
