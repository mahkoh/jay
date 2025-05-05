use {
    crate::{
        client::{Client, ClientId},
        criteria::{
            CritDestroyListener, CritMatcherId,
            tlm::{
                TL_CHANGED_APP_ID, TL_CHANGED_DESTROYED, TL_CHANGED_FLOATING,
                TL_CHANGED_FULLSCREEN, TL_CHANGED_NEW, TL_CHANGED_TITLE, TL_CHANGED_URGENT,
                TL_CHANGED_VISIBLE, TL_CHANGED_WORKSPACE, TlMatcherChange,
            },
        },
        ifs::{
            ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
            ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
            ext_image_copy::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
            jay_screencast::JayScreencast,
            jay_toplevel::JayToplevel,
            wl_seat::{NodeSeatState, SeatId, collect_kb_foci, collect_kb_foci2},
            wl_surface::{
                WlSurface, x_surface::xwindow::XwindowData,
                xdg_surface::xdg_toplevel::XdgToplevelToplevelData,
            },
            zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
            zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        },
        rect::Rect,
        state::State,
        tree::{
            ContainerNode, ContainerSplit, ContainingNode, Direction, Node, NodeId, OutputNode,
            PlaceholderNode, WorkspaceNode,
        },
        utils::{
            array_to_tuple::ArrayToTuple,
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
            numcell::NumCell,
            threshold_counter::ThresholdCounter,
            toplevel_identifier::{ToplevelIdentifier, toplevel_identifier},
        },
        wire::{
            ExtForeignToplevelHandleV1Id, ExtImageCopyCaptureSessionV1Id, JayScreencastId,
            JayToplevelId, ZwlrForeignToplevelHandleV1Id,
        },
    },
    jay_config::{window, window::WindowType},
    std::{
        borrow::Borrow,
        cell::{Cell, RefCell},
        ops::Deref,
        rc::{Rc, Weak},
    },
};

tree_id!(ToplevelNodeId);

pub trait ToplevelNode: ToplevelNodeBase {
    fn tl_surface_active_changed(&self, active: bool);
    fn tl_set_fullscreen(self: Rc<Self>, fullscreen: bool, ws: Option<Rc<WorkspaceNode>>);
    fn tl_title_changed(&self);
    fn tl_set_parent(&self, parent: Rc<dyn ContainingNode>);
    fn tl_extents_changed(&self);
    fn tl_set_workspace(&self, ws: &Rc<WorkspaceNode>);
    fn tl_workspace_output_changed(&self, prev: &Rc<OutputNode>, new: &Rc<OutputNode>);
    fn tl_change_extents(self: Rc<Self>, rect: &Rect);
    fn tl_set_visible(&self, visible: bool);
    fn tl_destroy(&self);
    fn tl_pinned(&self) -> bool;
    fn tl_set_pinned(&self, self_pinned: bool, pinned: bool);
}

impl<T: ToplevelNodeBase> ToplevelNode for T {
    fn tl_surface_active_changed(&self, active: bool) {
        let data = self.tl_data();
        data.update_active(self, || {
            data.active_surfaces.adj(active);
        });
    }

    fn tl_set_fullscreen(self: Rc<Self>, fullscreen: bool, ws: Option<Rc<WorkspaceNode>>) {
        let data = self.tl_data();
        if fullscreen {
            if let Some(ws) = ws.or_else(|| data.workspace.get()) {
                data.set_fullscreen2(&data.state, self.clone(), &ws);
            }
        } else {
            data.unset_fullscreen(&data.state, self.clone());
        }
    }

    fn tl_title_changed(&self) {
        let data = self.tl_data();
        let title = data.title.borrow_mut();
        if let Some(parent) = data.parent.get() {
            parent.node_child_title_changed(self, &title);
        }
        if let Some(data) = data.fullscrceen_data.borrow_mut().deref() {
            data.placeholder
                .tl_data()
                .title
                .borrow_mut()
                .clone_from(&title);
            data.placeholder.tl_title_changed();
        }
        data.property_changed(TL_CHANGED_TITLE);
    }

    fn tl_set_parent(&self, parent: Rc<dyn ContainingNode>) {
        let data = self.tl_data();
        let parent_was_none = data.parent.set(Some(parent.clone())).is_none();
        if parent_was_none {
            data.mapped_during_iteration.set(data.state.eng.iteration());
            data.property_changed(TL_CHANGED_NEW);
        }
        let was_floating = data.is_floating.get();
        let is_floating = parent.node_is_float();
        if was_floating != is_floating {
            data.property_changed(TL_CHANGED_FLOATING);
        }
        data.is_floating.set(is_floating);
        self.tl_set_workspace(&parent.cnode_workspace());
    }

    fn tl_extents_changed(&self) {
        let data = self.tl_data();
        if let Some(parent) = data.parent.get() {
            let pos = data.pos.get();
            parent.node_child_size_changed(self, pos.width(), pos.height());
            data.state.tree_changed();
        }
    }

    fn tl_set_workspace(&self, ws: &Rc<WorkspaceNode>) {
        let data = self.tl_data();
        let prev = data.workspace.set(Some(ws.clone()));
        self.tl_set_workspace_ext(ws);
        self.tl_data().property_changed(TL_CHANGED_WORKSPACE);
        let prev_output = match &prev {
            Some(n) => n.output.get(),
            _ => ws.state.dummy_output.get().unwrap(),
        };
        let new_output = ws.output.get();
        if prev.is_none() || prev_output.id != new_output.id {
            self.tl_workspace_output_changed(&prev_output, &new_output);
        }
    }

    fn tl_workspace_output_changed(&self, prev: &Rc<OutputNode>, new: &Rc<OutputNode>) {
        let data = self.tl_data();
        for sc in data.jay_screencasts.lock().values() {
            sc.update_latch_listener();
        }
        for sc in data.ext_copy_sessions.lock().values() {
            sc.update_latch_listener();
        }
        if prev.id != new.id {
            for handle in data.manager_handles.borrow().lock().values() {
                handle.leave_output(prev);
                handle.enter_output(new);
                handle.send_done();
            }
        }
    }

    fn tl_change_extents(self: Rc<Self>, rect: &Rect) {
        let data = self.tl_data();
        let prev = data.desired_extents.replace(*rect);
        if prev.size() != rect.size() {
            for sc in data.jay_screencasts.lock().values() {
                sc.schedule_realloc_or_reconfigure();
            }
            for sc in data.ext_copy_sessions.lock().values() {
                sc.buffer_size_changed();
            }
        }
        if data.is_floating.get() {
            data.float_width.set(rect.width());
            data.float_height.set(rect.height());
        }
        self.tl_change_extents_impl(rect)
    }

    fn tl_set_visible(&self, visible: bool) {
        self.tl_set_visible_impl(visible);
        self.tl_data().set_visible(self, visible);
    }

    fn tl_destroy(&self) {
        self.tl_data().destroy_node(self);
        self.tl_destroy_impl();
    }

    fn tl_pinned(&self) -> bool {
        let Some(parent) = self.tl_data().parent.get() else {
            return false;
        };
        parent.cnode_pinned()
    }

    fn tl_set_pinned(&self, self_pinned: bool, pinned: bool) {
        let data = self.tl_data();
        if self_pinned {
            data.pinned.set(pinned);
        }
        let Some(parent) = data.parent.get() else {
            return;
        };
        parent.cnode_set_pinned(pinned);
    }
}

pub trait ToplevelNodeBase: Node {
    fn tl_data(&self) -> &ToplevelData;

    fn tl_accepts_keyboard_focus(&self) -> bool {
        true
    }

    fn tl_set_active(&self, active: bool) {
        let _ = active;
    }

    fn tl_focus_child(&self) -> Option<Rc<dyn Node>> {
        None
    }

    fn tl_set_workspace_ext(&self, ws: &Rc<WorkspaceNode>) {
        let _ = ws;
    }

    fn tl_change_extents_impl(self: Rc<Self>, rect: &Rect);

    fn tl_close(self: Rc<Self>);

    fn tl_set_visible_impl(&self, visible: bool);
    fn tl_destroy_impl(&self);

    fn tl_last_active_child(self: Rc<Self>) -> Rc<dyn ToplevelNode>;

    fn tl_scanout_surface(&self) -> Option<Rc<WlSurface>> {
        None
    }
    fn tl_restack_popups(&self) {
        // nothing
    }

    fn tl_admits_children(&self) -> bool;

    fn tl_tile_drag_destination(
        self: Rc<Self>,
        source: NodeId,
        split: Option<ContainerSplit>,
        abs_bounds: Rect,
        abs_x: i32,
        abs_y: i32,
    ) -> Option<TileDragDestination>;

    fn tl_tile_drag_bounds(&self, split: ContainerSplit, start: bool) -> i32 {
        let _ = start;
        default_tile_drag_bounds(self, split)
    }

    fn tl_render_bounds(&self) -> Option<Rect> {
        self.tl_data()
            .parent
            .is_some()
            .then_some(self.node_absolute_position())
    }
}

pub struct FullscreenedData {
    pub placeholder: Rc<PlaceholderNode>,
    pub workspace: Rc<WorkspaceNode>,
}

#[derive(Clone)]
pub struct ToplevelOpt {
    toplevel: Weak<dyn ToplevelNode>,
    identifier: ToplevelIdentifier,
}

impl ToplevelOpt {
    pub fn get(&self) -> Option<Rc<dyn ToplevelNode>> {
        let tl = self.toplevel.upgrade()?;
        if tl.tl_data().identifier.get() == self.identifier {
            Some(tl)
        } else {
            None
        }
    }
}

pub enum ToplevelType {
    Container,
    Placeholder(Option<ToplevelIdentifier>),
    XdgToplevel(Rc<XdgToplevelToplevelData>),
    XWindow(Rc<XwindowData>),
}

impl ToplevelType {
    pub fn to_window_type(&self) -> WindowType {
        match self {
            ToplevelType::Container => window::CONTAINER,
            ToplevelType::Placeholder { .. } => window::PLACEHOLDER,
            ToplevelType::XdgToplevel { .. } => window::XDG_TOPLEVEL,
            ToplevelType::XWindow { .. } => window::X_WINDOW,
        }
    }
}

pub struct ToplevelData {
    pub node_id: NodeId,
    pub kind: ToplevelType,
    pub self_active: Cell<bool>,
    pub client: Option<Rc<Client>>,
    pub state: Rc<State>,
    pub active_surfaces: ThresholdCounter,
    pub visible: Cell<bool>,
    pub is_floating: Cell<bool>,
    pub float_width: Cell<i32>,
    pub float_height: Cell<i32>,
    pub pinned: Cell<bool>,
    pub is_fullscreen: Cell<bool>,
    pub fullscrceen_data: RefCell<Option<FullscreenedData>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub title: RefCell<String>,
    pub parent: CloneCell<Option<Rc<dyn ContainingNode>>>,
    pub mapped_during_iteration: Cell<u64>,
    pub pos: Cell<Rect>,
    pub desired_extents: Cell<Rect>,
    pub seat_state: NodeSeatState,
    pub wants_attention: Cell<bool>,
    pub requested_attention: Cell<bool>,
    pub app_id: RefCell<String>,
    pub identifier: Cell<ToplevelIdentifier>,
    pub handles:
        CopyHashMap<(ClientId, ExtForeignToplevelHandleV1Id), Rc<ExtForeignToplevelHandleV1>>,
    pub manager_handles:
        CopyHashMap<(ClientId, ZwlrForeignToplevelHandleV1Id), Rc<ZwlrForeignToplevelHandleV1>>,
    pub render_highlight: NumCell<u32>,
    pub jay_toplevels: CopyHashMap<(ClientId, JayToplevelId), Rc<JayToplevel>>,
    pub jay_screencasts: CopyHashMap<(ClientId, JayScreencastId), Rc<JayScreencast>>,
    pub ext_copy_sessions:
        CopyHashMap<(ClientId, ExtImageCopyCaptureSessionV1Id), Rc<ExtImageCopyCaptureSessionV1>>,
    pub slf: Weak<dyn ToplevelNode>,
    pub destroyed: CopyHashMap<CritMatcherId, Weak<dyn CritDestroyListener<ToplevelData>>>,
    pub changed_properties: Cell<TlMatcherChange>,
    pub just_mapped_scheduled: Cell<bool>,
    pub seat_foci: CopyHashMap<SeatId, ()>,
}

impl ToplevelData {
    pub fn new<T: ToplevelNode>(
        state: &Rc<State>,
        title: String,
        client: Option<Rc<Client>>,
        kind: ToplevelType,
        node_id: impl Into<NodeId>,
        slf: &Weak<T>,
    ) -> Self {
        let node_id = node_id.into();
        let id = toplevel_identifier();
        state.toplevels.set(id, slf.clone());
        Self {
            node_id,
            kind,
            self_active: Cell::new(false),
            client,
            state: state.clone(),
            active_surfaces: Default::default(),
            visible: Cell::new(false),
            is_floating: Default::default(),
            float_width: Default::default(),
            float_height: Default::default(),
            pinned: Cell::new(false),
            is_fullscreen: Default::default(),
            fullscrceen_data: Default::default(),
            workspace: Default::default(),
            title: RefCell::new(title),
            parent: Default::default(),
            mapped_during_iteration: Cell::new(0),
            pos: Default::default(),
            desired_extents: Default::default(),
            seat_state: Default::default(),
            wants_attention: Cell::new(false),
            requested_attention: Cell::new(false),
            app_id: Default::default(),
            identifier: Cell::new(id),
            handles: Default::default(),
            manager_handles: Default::default(),
            render_highlight: Default::default(),
            jay_toplevels: Default::default(),
            jay_screencasts: Default::default(),
            ext_copy_sessions: Default::default(),
            slf: slf.clone(),
            destroyed: Default::default(),
            changed_properties: Default::default(),
            just_mapped_scheduled: Cell::new(false),
            seat_foci: Default::default(),
        }
    }

    pub fn active(&self) -> bool {
        self.active_surfaces.active() || self.self_active.get()
    }

    fn update_active<T: ToplevelNode, F: FnOnce()>(&self, tl: &T, f: F) {
        let active_old = self.active();
        f();
        let active_new = self.active();
        if active_old != active_new {
            tl.tl_set_active(active_new);
            if let Some(parent) = self.parent.get() {
                parent.node_child_active_changed(tl, active_new, 1);
            }
            for handle in self.manager_handles.borrow().lock().values() {
                handle.send_state(active_new, self.is_fullscreen.get());
                handle.send_done();
            }
        }
    }

    pub fn update_self_active<T: ToplevelNode>(&self, node: &T, active: bool) {
        self.update_active(node, || self.self_active.set(active));
    }

    pub fn float_size(&self, ws: &WorkspaceNode) -> (i32, i32) {
        let output = ws.output.get().global.pos.get();
        let mut width = self.float_width.get();
        let mut height = self.float_height.get();
        if width == 0 {
            width = output.width() / 2;
        }
        if height == 0 {
            height = output.height() / 2;
        }
        (width, height)
    }

    pub fn property_changed(&self, change: TlMatcherChange) {
        let mgr = &self.state.tl_matcher_manager;
        let props = self.changed_properties.get();
        if props.is_none() && mgr.has_no_interest(self, change) {
            return;
        }
        self.changed_properties.set(props | change);
        if props.is_none() && change.is_some() {
            if let Some(node) = self.slf.upgrade() {
                mgr.changed(node);
            }
        }
    }

    pub fn destroy_node(&self, node: &dyn Node) {
        for jay_tl in self.jay_toplevels.lock().drain_values() {
            jay_tl.destroy();
        }
        for screencast in self.jay_screencasts.lock().drain_values() {
            screencast.do_destroy();
        }
        for screencast in self.ext_copy_sessions.lock().drain_values() {
            screencast.stop();
        }
        {
            let id = toplevel_identifier();
            let prev = self.identifier.replace(id);
            self.state.remove_toplevel_id(prev);
            self.state.toplevels.set(id, self.slf.clone());
        }
        {
            let mut handles = self.handles.lock();
            for handle in handles.drain_values() {
                handle.send_closed();
            }
        }
        {
            let mut manager_handles = self.manager_handles.lock();
            for handle in manager_handles.drain_values() {
                handle.send_closed();
            }
        }
        self.detach_node(node);
        self.property_changed(TL_CHANGED_DESTROYED);
    }

    pub fn detach_node(&self, node: &dyn Node) {
        if let Some(fd) = self.fullscrceen_data.borrow_mut().take() {
            fd.placeholder.tl_destroy();
        }
        if let Some(parent) = self.parent.take() {
            parent.cnode_remove_child(node);
        }
        self.workspace.take();
        self.seat_state.destroy_node(node);
    }

    pub fn broadcast(&self, toplevel: Rc<dyn ToplevelNode>) {
        let id = self.identifier.get().to_string();
        let title = self.title.borrow();
        let app_id = self.app_id.borrow();
        let activated = self.active();
        let fullscreen = self.is_fullscreen.get();
        let class;
        let manager_app_id = match &self.kind {
            ToplevelType::XWindow(w) => {
                class = w.info.class.borrow();
                class.as_deref().unwrap_or_default()
            }
            _ => &app_id,
        };
        for list in self.state.toplevel_lists.lock().values() {
            self.send_once(&toplevel, list, &id, &title, &app_id);
        }
        for manager in self.state.toplevel_managers.lock().values() {
            self.manager_send_once(
                &toplevel,
                manager,
                &title,
                manager_app_id,
                activated,
                fullscreen,
            );
        }
    }

    pub fn send(&self, toplevel: Rc<dyn ToplevelNode>, list: &ExtForeignToplevelListV1) {
        let id = self.identifier.get().to_string();
        let title = self.title.borrow();
        let app_id = self.app_id.borrow();
        self.send_once(&toplevel, list, &id, &title, &app_id);
    }

    fn send_once(
        &self,
        toplevel: &Rc<dyn ToplevelNode>,
        list: &ExtForeignToplevelListV1,
        id: &str,
        title: &str,
        app_id: &str,
    ) {
        let opt = ToplevelOpt {
            toplevel: Rc::downgrade(toplevel),
            identifier: self.identifier.get(),
        };
        let handle = match list.publish_toplevel(opt) {
            None => return,
            Some(handle) => handle,
        };
        handle.send_identifier(id);
        handle.send_title(title);
        handle.send_app_id(app_id);
        handle.send_done();
        self.handles
            .set((handle.client.id, handle.id), handle.clone());
    }

    pub fn manager_send(
        &self,
        toplevel: Rc<dyn ToplevelNode>,
        manager: &ZwlrForeignToplevelManagerV1,
    ) {
        let title = self.title.borrow();
        let activated = self.active();
        let fullscreen = self.is_fullscreen.get();
        let app_id;
        let class;
        let manager_app_id = match &self.kind {
            ToplevelType::XWindow(w) => {
                class = w.info.class.borrow();
                class.as_deref().unwrap_or_default()
            }
            _ => {
                app_id = self.app_id.borrow();
                &app_id
            }
        };
        self.manager_send_once(
            &toplevel,
            manager,
            &title,
            manager_app_id,
            activated,
            fullscreen,
        );
    }

    fn manager_send_once(
        &self,
        toplevel: &Rc<dyn ToplevelNode>,
        manager: &ZwlrForeignToplevelManagerV1,
        title: &str,
        app_id: &str,
        activated: bool,
        fullscreen: bool,
    ) {
        let opt = ToplevelOpt {
            toplevel: Rc::downgrade(toplevel),
            identifier: self.identifier.get(),
        };
        let handle = match manager.publish_toplevel(opt) {
            None => return,
            Some(handle) => handle,
        };
        handle.send_app_id(app_id);
        handle.send_title(title);
        handle.enter_output(&self.output());
        handle.send_state(activated, fullscreen);
        handle.send_done();
        self.manager_handles
            .set((handle.client.id, handle.id), handle.clone());
    }

    pub fn set_title(&self, title: &str) {
        *self.title.borrow_mut() = title.to_string();
        for handle in self.handles.lock().values() {
            handle.send_title(title);
            handle.send_done();
        }
        for handle in self.manager_handles.lock().values() {
            handle.send_title(title);
            handle.send_done();
        }
    }

    pub fn set_app_id(&self, app_id: &str) {
        let dst = &mut *self.app_id.borrow_mut();
        if *dst == app_id {
            return;
        }
        *dst = app_id.to_string();
        for handle in self.handles.lock().values() {
            handle.send_app_id(app_id);
            handle.send_done();
        }
        for handle in self.manager_handles.lock().values() {
            handle.send_app_id(app_id);
            handle.send_done();
        }
        self.property_changed(TL_CHANGED_APP_ID)
    }

    pub fn set_fullscreen(
        &self,
        state: &Rc<State>,
        node: Rc<dyn ToplevelNode>,
        output: &Rc<OutputNode>,
    ) {
        self.set_fullscreen2(state, node, &output.ensure_workspace());
    }

    pub fn set_fullscreen2(
        &self,
        state: &Rc<State>,
        node: Rc<dyn ToplevelNode>,
        ws: &Rc<WorkspaceNode>,
    ) {
        if ws.fullscreen.is_some() {
            log::info!(
                "Cannot fullscreen a node on a workspace that already has a fullscreen node attached"
            );
            return;
        }
        if node.node_is_placeholder() {
            log::info!("Cannot fullscreen a placeholder node");
            return;
        }
        let mut data = self.fullscrceen_data.borrow_mut();
        if data.is_some() {
            log::info!("Cannot fullscreen a node that is already fullscreen");
            return;
        }
        let parent = match node.tl_data().parent.get() {
            None => {
                log::warn!("Cannot fullscreen a node without a parent");
                return;
            }
            Some(p) => p,
        };
        if parent.node_is_workspace() {
            log::warn!("Cannot fullscreen root container in a workspace");
            return;
        }
        let placeholder =
            Rc::new_cyclic(|weak| PlaceholderNode::new_for(state, node.clone(), weak));
        parent.cnode_replace_child(&*node, placeholder.clone());
        let mut kb_foci = Default::default();
        if ws.visible.get() {
            if let Some(container) = ws.container.get() {
                kb_foci = collect_kb_foci(container);
            }
            for stacked in ws.stacked.iter() {
                collect_kb_foci2(stacked.deref().clone(), &mut kb_foci);
            }
        }
        *data = Some(FullscreenedData {
            placeholder,
            workspace: ws.clone(),
        });
        drop(data);
        self.is_fullscreen.set(true);
        self.property_changed(TL_CHANGED_FULLSCREEN);
        node.tl_set_parent(ws.clone());
        ws.set_fullscreen_node(&node);
        node.clone()
            .tl_change_extents(&ws.output.get().global.pos.get());
        for seat in kb_foci {
            node.clone().node_do_focus(&seat, Direction::Unspecified);
        }
        for handle in self.manager_handles.lock().values() {
            handle.send_state(self.active(), true);
            handle.send_done();
        }
    }

    pub fn unset_fullscreen(&self, state: &Rc<State>, node: Rc<dyn ToplevelNode>) {
        if !self.is_fullscreen.get() {
            log::warn!("Cannot unset fullscreen on a node that is not fullscreen");
            return;
        }
        let fd = match self.fullscrceen_data.borrow_mut().take() {
            Some(fd) => fd,
            _ => {
                log::error!("is_fullscreen = true but data is None");
                return;
            }
        };
        self.is_fullscreen.set(false);
        self.property_changed(TL_CHANGED_FULLSCREEN);
        match fd.workspace.fullscreen.get() {
            None => {
                log::error!(
                    "Node is supposed to be fullscreened on a workspace but workspace has not fullscreen node."
                );
                return;
            }
            Some(f) if f.node_id() != node.node_id() => {
                log::error!(
                    "Node is supposed to be fullscreened on a workspace but the workspace has a different node attached."
                );
                return;
            }
            _ => {}
        }
        fd.workspace.remove_fullscreen_node();
        if fd.placeholder.is_destroyed() {
            state.map_tiled(node);
            return;
        }
        let parent = fd.placeholder.tl_data().parent.take().unwrap();
        parent.cnode_replace_child(fd.placeholder.deref(), node.clone());
        if node.node_visible() {
            let kb_foci = collect_kb_foci(fd.placeholder.clone());
            for seat in kb_foci {
                node.clone().node_do_focus(&seat, Direction::Unspecified);
            }
        }
        fd.placeholder.tl_destroy();
        for handle in self.manager_handles.lock().values() {
            handle.send_state(self.active(), false);
            handle.send_done();
        }
    }

    pub fn set_visible(&self, node: &dyn Node, visible: bool) {
        if self.visible.replace(visible) != visible {
            self.property_changed(TL_CHANGED_VISIBLE);
        }
        self.seat_state.set_visible(node, visible);
        for sc in self.jay_screencasts.lock().values() {
            sc.update_latch_listener();
        }
        for sc in self.ext_copy_sessions.lock().values() {
            sc.update_latch_listener();
        }
        if !visible {
            return;
        }
        if !self.requested_attention.replace(false) {
            return;
        }
        self.set_wants_attention(false);
        if let Some(parent) = self.parent.get() {
            parent.cnode_child_attention_request_changed(node, false);
        }
    }

    pub fn request_attention(&self, node: &dyn Node) {
        if self.visible.get() {
            return;
        }
        if self.requested_attention.replace(true) {
            return;
        }
        self.set_wants_attention(true);
        if let Some(parent) = self.parent.get() {
            parent.cnode_child_attention_request_changed(node, true);
        }
    }

    pub fn set_wants_attention(&self, value: bool) {
        if self.wants_attention.replace(value) != value {
            self.property_changed(TL_CHANGED_URGENT);
        }
    }

    pub fn output(&self) -> Rc<OutputNode> {
        match self.output_opt() {
            None => self.state.dummy_output.get().unwrap(),
            Some(o) => o,
        }
    }

    pub fn output_opt(&self) -> Option<Rc<OutputNode>> {
        self.workspace.get().map(|ws| ws.output.get())
    }

    pub fn desired_pixel_size(&self) -> (i32, i32) {
        let (dw, dh) = self.desired_extents.get().size();
        if let Some(ws) = self.workspace.get() {
            let scale = ws.output.get().global.persistent.scale.get();
            return scale.pixel_size([dw, dh]).to_tuple();
        };
        (0, 0)
    }

    pub fn just_mapped(&self) -> bool {
        self.mapped_during_iteration.get() == self.state.eng.iteration()
    }
}

impl Drop for ToplevelData {
    fn drop(&mut self) {
        self.state.remove_toplevel_id(self.identifier.get());
    }
}

pub struct TileDragDestination {
    pub highlight: Rect,
    pub ty: TddType,
}

pub enum TddType {
    Replace(Rc<dyn ToplevelNode>),
    Split {
        node: Rc<dyn ToplevelNode>,
        split: ContainerSplit,
        before: bool,
    },
    Insert {
        container: Rc<ContainerNode>,
        neighbor: Rc<dyn ToplevelNode>,
        before: bool,
    },
    NewWorkspace {
        output: Rc<OutputNode>,
    },
    NewContainer {
        workspace: Rc<WorkspaceNode>,
    },
    MoveToWorkspace {
        workspace: Rc<WorkspaceNode>,
    },
    MoveToNewWorkspace {
        output: Rc<OutputNode>,
    },
}

pub fn default_tile_drag_bounds<T: ToplevelNodeBase + ?Sized>(t: &T, split: ContainerSplit) -> i32 {
    const FACTOR: i32 = 5;
    match split {
        ContainerSplit::Horizontal => t.node_absolute_position().width() / FACTOR,
        ContainerSplit::Vertical => t.node_absolute_position().height() / FACTOR,
    }
}

pub fn toplevel_parent_container(tl: &dyn ToplevelNode) -> Option<Rc<ContainerNode>> {
    if let Some(parent) = tl.tl_data().parent.get() {
        if let Some(container) = parent.node_into_container() {
            return Some(container);
        }
    }
    None
}

pub fn toplevel_create_split(state: &Rc<State>, tl: Rc<dyn ToplevelNode>, axis: ContainerSplit) {
    if tl.tl_data().is_fullscreen.get() {
        return;
    }
    let ws = match tl.tl_data().workspace.get() {
        Some(ws) => ws,
        _ => return,
    };
    let pn = match tl.tl_data().parent.get() {
        Some(pn) => pn,
        _ => return,
    };
    if let Some(pn) = pn.node_into_containing_node() {
        let cn = ContainerNode::new(state, &ws, tl.clone(), axis);
        pn.cnode_replace_child(&*tl, cn);
    }
}

pub fn toplevel_set_floating(state: &Rc<State>, tl: Rc<dyn ToplevelNode>, floating: bool) {
    let data = tl.tl_data();
    if data.is_fullscreen.get() {
        return;
    }
    if data.is_floating.get() == floating {
        return;
    }
    let parent = match data.parent.get() {
        Some(p) => p,
        _ => return,
    };
    if !floating {
        parent.cnode_remove_child2(&*tl, true);
        state.map_tiled(tl);
    } else if let Some(ws) = data.workspace.get() {
        parent.cnode_remove_child2(&*tl, true);
        let (width, height) = data.float_size(&ws);
        state.map_floating(tl, width, height, &ws, None);
    }
}

pub fn toplevel_set_workspace(state: &Rc<State>, tl: Rc<dyn ToplevelNode>, ws: &Rc<WorkspaceNode>) {
    if tl.tl_data().is_fullscreen.get() {
        return;
    }
    let old_ws = match tl.tl_data().workspace.get() {
        Some(ws) => ws,
        _ => return,
    };
    if old_ws.id == ws.id {
        return;
    }
    let cn = match tl.tl_data().parent.get() {
        Some(cn) => cn,
        _ => return,
    };
    let kb_foci = collect_kb_foci(tl.clone());
    cn.cnode_remove_child2(&*tl, true);
    if !ws.visible.get() {
        for focus in kb_foci {
            old_ws.clone().node_do_focus(&focus, Direction::Unspecified);
        }
    }
    if tl.tl_data().is_floating.get() {
        let (width, height) = tl.tl_data().float_size(ws);
        state.map_floating(tl.clone(), width, height, ws, None);
    } else {
        state.map_tiled_on(tl, ws);
    }
}
