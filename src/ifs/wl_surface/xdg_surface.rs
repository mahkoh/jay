pub mod xdg_popup;
pub mod xdg_toplevel;

use {
    crate::{
        client::ClientError,
        configurable::{Configurable, ConfigurableData, ConfigurableDataCore, ConfigurableExt},
        ifs::{
            wl_surface::{
                CommitAction, PendingState, SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError,
                tray::TrayItemId,
                xdg_surface::{
                    xdg_popup::{XdgPopup, XdgPopupError, XdgPopupParent},
                    xdg_toplevel::{WM_CAPABILITIES_SINCE, XdgToplevel},
                },
            },
            xdg_wm_base::XdgWmBase,
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        tree::{
            FindTreeResult, FoundNode, Node, NodeBase, NodeLayerLink, NodeLocation, NodesStack,
            NodesStackElement, OutputNode, SplitView, StackedNode, TreeSerial,
            TreeTimeline::{self, LiveTL, RenderTL},
            WorkspaceNode, WorkspaceType,
        },
        utils::{
            box_cache::{BoxReset, CachedBox},
            cell_ext::CellExt,
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
            linkedlist::LinkedNode,
        },
        wire::{WlSurfaceId, XdgPopupId, XdgSurfaceId, xdg_surface::*},
    },
    jay_proc::Reset,
    std::{
        cell::{Cell, RefCell, RefMut},
        fmt::Debug,
        rc::Rc,
    },
    thiserror::Error,
};

#[expect(dead_code)]
const NOT_CONSTRUCTED: u32 = 1;
const ALREADY_CONSTRUCTED: u32 = 2;
#[expect(dead_code)]
const UNCONFIGURED_BUFFER: u32 = 3;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum XdgSurfaceRole {
    None,
    XdgPopup,
    XdgToplevel,
}

impl XdgSurfaceRole {
    fn name(self) -> &'static str {
        match self {
            XdgSurfaceRole::None => "none",
            XdgSurfaceRole::XdgPopup => "xdg_popup",
            XdgSurfaceRole::XdgToplevel => "xdg_toplevel",
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PopupStackType {
    Normal,
    AboveLayers,
    Overlay,
}

pub struct XdgSurface {
    id: XdgSurfaceId,
    base: Rc<XdgWmBase>,
    role: Cell<XdgSurfaceRole>,
    pub surface: Rc<WlSurface>,
    acked_serial: Cell<Option<TreeSerial>>,
    geometry: Cell<Option<Rect>>,
    extents: Cell<Rect>,
    effective_geometry: SplitView<Cell<Rect>>,
    pub absolute_desired_extents: SplitView<Cell<Rect>>,
    ext: CloneCell<Option<Rc<dyn XdgSurfaceExt>>>,
    popup_display_stack: CloneCell<Rc<NodesStack>>,
    popup_stack_type: Cell<PopupStackType>,
    popups: CopyHashMap<XdgPopupId, Rc<Popup>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    workspace_type: Cell<Option<WorkspaceType>>,
    pub tracker: Tracker<Self>,
    initial_commit_state: Cell<InitialCommitState>,
    destroyed: Cell<bool>,
    configure_data: ConfigurableData<XdgSurfaceConfigureData>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
enum InitialCommitState {
    #[default]
    Unmapped,
    Sent,
    Mapped,
}

struct Popup {
    parent: Rc<XdgSurface>,
    popup: Rc<XdgPopup>,
    display_link: RefCell<NodesStackElement>,
    workspace_link: RefCell<Option<LinkedNode<Rc<dyn StackedNode>>>>,
}

impl XdgPopupParent for Popup {
    fn position(&self) -> Rect {
        self.parent.absolute_desired_extents[LiveTL].get()
    }

    fn remove_popup(&self) {
        self.parent.popups.remove(&self.popup.id);
    }

    fn output(&self) -> Rc<OutputNode> {
        self.parent.surface.output.get()
    }

    fn has_workspace_link(&self) -> bool {
        self.workspace_link.borrow().is_some()
    }

    fn post_commit(&self) {
        let mut wl = self.workspace_link.borrow_mut();
        let mut dl = self.display_link.borrow_mut();
        let surface = &self.popup.xdg.surface;
        let state = &surface.client.state;
        if surface.buffer.is_some() {
            let mut any_set = false;
            if wl.is_none()
                && let Some(ws) = self.parent.workspace.get()
            {
                self.popup.xdg.set_workspace(&ws);
                *wl = Some(ws.stacked.add_last(self.popup.clone()));
                any_set = true;
            }
            if dl.link.is_none() {
                dl.link = Some(dl.stack.stacked.add_last(self.popup.clone()));
                any_set = true;
            }
            if any_set {
                state.tree_changed();
                drop(dl);
                self.popup
                    .set_visible(self.parent.surface.visible[LiveTL].get());
            }
        } else {
            if wl.take().is_some() {
                drop(wl);
                drop(dl);
                self.popup.set_visible(false);
                self.popup.destroy_node();
            }
        }
    }

    fn visible(&self) -> bool {
        self.parent.surface.visible[LiveTL].get()
    }

    fn make_visible(self: Rc<Self>) {
        if let Some(ext) = self.parent.ext.get() {
            ext.node_make_visible_dyn();
        }
    }

    fn node_layer(&self) -> NodeLayerLink {
        let Some(link) = self.display_link.borrow().link.as_ref().map(|w| w.to_ref()) else {
            return NodeLayerLink::Display;
        };
        match self.popup.xdg.popup_stack_type.get() {
            PopupStackType::Normal => NodeLayerLink::Stacked(link),
            PopupStackType::AboveLayers => NodeLayerLink::StackedAboveLayers(link),
            PopupStackType::Overlay => NodeLayerLink::OverlayStacked(link),
        }
    }

    fn nodes_stack_element(&self) -> &RefCell<NodesStackElement> {
        &self.display_link
    }

    fn tray_item(&self) -> Option<TrayItemId> {
        self.parent.clone().tray_item()
    }
}

#[derive(Default, Debug, Reset)]
pub struct PendingXdgSurfaceData {
    geometry: Option<Rect>,
    pub restored: Option<Rc<Cell<bool>>>,
    min_size: Option<(Option<i32>, Option<i32>)>,
    max_size: Option<(Option<i32>, Option<i32>)>,
}

impl PendingXdgSurfaceData {
    pub fn merge(&mut self, next: &mut Self) {
        macro_rules! opt {
            ($name:ident) => {
                if let Some(n) = next.$name.take() {
                    self.$name = Some(n);
                }
            };
        }
        opt!(geometry);
        opt!(restored);
        opt!(min_size);
        opt!(max_size);
    }
}

trait XdgSurfaceExt: Node + Debug {
    fn initial_configure(self: Rc<Self>) {
        // nothing
    }

    fn commit_requested(&self) {
        // nothing
    }

    fn before_apply_commit(
        self: Rc<Self>,
        pending: &mut PendingState,
    ) -> Result<(), WlSurfaceError> {
        let _ = pending;
        Ok(())
    }

    fn post_commit(self: Rc<Self>) {
        // nothing
    }

    fn extents_changed(&self) {
        // nothing
    }

    fn geometry_changed(&self) {
        // nothing
    }

    fn focus_node(&self) -> Option<Rc<dyn Node>> {
        None
    }

    fn tray_item(&self) -> Option<TrayItemId> {
        None
    }

    fn effective_geometry(&self, geometry: Rect, tl: TreeTimeline) -> Rect {
        let _ = tl;
        geometry
    }

    fn schedule_xdg_op(self: Rc<Self>, op: XdgSurfaceTransactionOp);

    fn configure_data(&self) -> XdgSurfaceConfigureData;

    fn send_configure(&self, data: XdgSurfaceConfigureData);
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct XdgToplevelConfigureData {
    pub w: i32,
    pub h: i32,
    pub state: u32,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct XdgPopupConfigureData {
    pub repositioned: Option<u32>,
    pub rect: Rect,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum XdgSurfaceConfigureData {
    None,
    Toplevel(Option<XdgToplevelConfigureData>),
    Popup(XdgPopupConfigureData),
}

impl XdgSurface {
    pub fn new(wm_base: &Rc<XdgWmBase>, id: XdgSurfaceId, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
            base: wm_base.clone(),
            role: Cell::new(XdgSurfaceRole::None),
            surface: surface.clone(),
            acked_serial: Default::default(),
            geometry: Cell::new(None),
            extents: Cell::new(surface.extents.get()),
            effective_geometry: Default::default(),
            absolute_desired_extents: Default::default(),
            ext: Default::default(),
            popup_display_stack: CloneCell::new(surface.client.state.root.stacked.clone()),
            popup_stack_type: Cell::new(PopupStackType::Normal),
            popups: Default::default(),
            workspace: Default::default(),
            workspace_type: Default::default(),
            tracker: Default::default(),
            initial_commit_state: Default::default(),
            destroyed: Default::default(),
            configure_data: ConfigurableData::new(&surface.client.state),
        }
    }

    fn update_surface_position(&self) {
        let (mut x1, mut y1) = self.absolute_desired_extents[LiveTL].get().position();
        let geo = self.effective_geometry[LiveTL].get();
        x1 -= geo.x1();
        y1 -= geo.y1();
        self.surface.set_absolute_position(x1, y1);
        self.update_popup_positions();
    }

    fn set_absolute_desired_extents(&self, rect: &Rect) {
        let prev = self.absolute_desired_extents[LiveTL].replace(*rect);
        if *rect != prev {
            if rect.position() != prev.position() {
                self.update_surface_position();
            }
            if let Some(ext) = self.ext.get() {
                ext.schedule_xdg_op(XdgSurfaceTransactionOp::SetAbsoluteDesiredExtents(*rect));
            }
        }
    }

    fn set_workspace(&self, ws: &Rc<WorkspaceNode>) {
        self.workspace.set(Some(ws.clone()));
        if self.workspace_type.replace(Some(ws.ty)) != Some(ws.ty) {
            let root = &self.surface.client.state.root;
            match ws.ty {
                WorkspaceType::Normal => {
                    self.set_popup_stack(&root.stacked, PopupStackType::Normal);
                }
                WorkspaceType::Overlay => {
                    self.set_popup_stack(&root.stacked_in_overlay, PopupStackType::Overlay);
                }
            }
        }
        self.surface
            .set_output(&ws.node_state[LiveTL].output.get(), ws.location());
        let pu = self.popups.lock();
        for pu in pu.values() {
            pu.popup.xdg.set_workspace(ws);
        }
    }

    pub fn set_output(&self, output: &Rc<OutputNode>) {
        self.surface
            .set_output(output, NodeLocation::Output(output.id));
        let pu = self.popups.lock();
        for pu in pu.values() {
            pu.popup.xdg.set_output(output);
        }
    }

    fn set_role(&self, role: XdgSurfaceRole) -> Result<(), XdgSurfaceError> {
        use XdgSurfaceRole::*;
        match (self.role.get(), role) {
            (None, _) => {}
            (old, new) if old == new => {}
            (old, new) => {
                return Err(XdgSurfaceError::IncompatibleRole {
                    id: self.id,
                    old,
                    new,
                });
            }
        }
        self.role.set(role);
        Ok(())
    }

    fn destroy_node(&self) {
        self.workspace.set(None);
        self.workspace_type.set(None);
        self.surface.destroy_node();
        for popup in self.popups.lock().drain_values() {
            popup.popup.destroy_node();
        }
        self.configure_data.ready();
    }

    fn detach_node(&self) {
        self.workspace.set(None);
        self.workspace_type.set(None);
        self.surface.detach_node(false);
        let popups = self.popups.lock();
        for popup in popups.values() {
            let _v = popup.workspace_link.borrow_mut().take();
            popup.popup.detach_node();
        }
    }

    pub fn damage(&self, tt: TreeTimeline) {
        let (x, y) = self.surface.buffer_abs_pos[tt].get().position();
        let extents = self.surface.extents.get();
        let rect = extents.move_(x, y);
        match tt {
            LiveTL => self.surface.damage(rect, LiveTL),
            RenderTL => self.surface.client.state.damage(rect),
        }
    }

    pub fn geometry(&self, tl: TreeTimeline) -> Rect {
        self.effective_geometry[tl].get()
    }

    pub fn send_configure(&self, serial: TreeSerial) {
        self.surface.client.event(Configure {
            self_id: self.id,
            serial: serial.raw() as _,
        })
    }

    pub fn install(self: &Rc<Self>) -> Result<(), XdgSurfaceError> {
        self.surface.set_role(SurfaceRole::XdgSurface)?;
        if self.surface.ext.get().is_some() {
            return Err(XdgSurfaceError::AlreadyAttached(self.surface.id));
        }
        self.surface.ext.set(self.clone());
        Ok(())
    }

    fn pending(&self) -> RefMut<'_, PendingXdgSurfaceData> {
        RefMut::map(self.surface.pending.borrow_mut(), |p| &mut p.xdg_surface)
    }

    pub fn set_popup_stack(&self, stack: &Rc<NodesStack>, stack_type: PopupStackType) {
        if self.popup_stack_type.replace(stack_type) == stack_type {
            return;
        }
        self.popup_display_stack.set(stack.clone());
        for popup in self.popups.lock().values() {
            if popup.popup.xdg.surface.node_visible(RenderTL) {
                popup.popup.xdg.damage(RenderTL);
            }
            popup.display_link.borrow_mut().restack_on(stack);
            popup.popup.xdg.set_popup_stack(stack, stack_type);
        }
    }

    pub fn for_each_popup(&self, mut f: impl FnMut(&Rc<XdgPopup>)) {
        for popup in self.popups.lock().values() {
            f(&popup.popup);
        }
    }

    fn unset_ext(&self) {
        self.ext.set(None);
        self.configure_data.ready();
        self.surface.set_dummy_output();
    }

    fn run_op(&self, op: XdgSurfaceTransactionOp) {
        match op {
            XdgSurfaceTransactionOp::UpdateGeometry => {
                self.update_effective_geometry(UpdateGeometryReason::FullscreenRender);
            }
            XdgSurfaceTransactionOp::SetAbsoluteDesiredExtents(v) => {
                self.absolute_desired_extents[RenderTL].set(v);
            }
        }
    }
}

impl XdgSurfaceRequestHandler for XdgSurface {
    type Error = XdgSurfaceError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroyed.set(true);
        self.configure_data.ready();
        if self.ext.is_some() {
            return Err(XdgSurfaceError::RoleNotYetDestroyed(self.id));
        }
        {
            let children = self.popups.lock();
            if !children.is_empty() {
                return Err(XdgSurfaceError::PopupsNotYetDestroyed);
            }
        }
        self.surface.unset_ext();
        self.base.surfaces.remove(&self.id);
        self.surface.client.remove_obj(self)?;
        Ok(())
    }

    fn get_toplevel(&self, req: GetToplevel, slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.set_role(XdgSurfaceRole::XdgToplevel)?;
        if self.ext.is_some() {
            self.surface.client.protocol_error(
                self,
                ALREADY_CONSTRUCTED,
                &format!(
                    "wl_surface {} already has an assigned xdg_toplevel",
                    self.surface.id
                ),
            );
            return Err(XdgSurfaceError::AlreadyConstructed);
        }
        let toplevel = Rc::new_cyclic(|weak| XdgToplevel::new(req.id, slf, weak));
        track!(self.surface.client, toplevel);
        self.surface.client.add_client_obj(&toplevel)?;
        self.ext.set(Some(toplevel.clone()));
        if self.base.version >= WM_CAPABILITIES_SINCE {
            toplevel.send_wm_capabilities();
        }
        self.surface.set_toplevel(Some(toplevel));
        Ok(())
    }

    fn get_popup(&self, req: GetPopup, slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.set_role(XdgSurfaceRole::XdgPopup)?;
        let mut parent = None;
        if req.parent.is_some() {
            parent = Some(self.surface.client.lookup(req.parent)?);
        }
        let positioner = self.surface.client.lookup(req.positioner)?;
        if self.ext.is_some() {
            self.surface.client.protocol_error(
                self,
                ALREADY_CONSTRUCTED,
                &format!(
                    "wl_surface {} already has an assigned xdg_popup",
                    self.surface.id
                ),
            );
            return Err(XdgSurfaceError::AlreadyConstructed);
        }
        let popup = Rc::new(XdgPopup::new(req.id, slf, &positioner)?);
        track!(self.surface.client, popup);
        self.surface.client.add_client_obj(&popup)?;
        if let Some(parent) = &parent {
            let user = Rc::new(Popup {
                parent: parent.clone(),
                popup: popup.clone(),
                display_link: parent.popup_display_stack.get().element(),
                workspace_link: Default::default(),
            });
            popup.parent.set(Some(user.clone()));
            popup.xdg.set_popup_stack(
                &parent.popup_display_stack.get(),
                parent.popup_stack_type.get(),
            );
            popup.xdg.set_output(&parent.surface.output.get());
            parent.popups.set(req.id, user);
        }
        self.ext.set(Some(popup));
        Ok(())
    }

    fn set_window_geometry(
        &self,
        req: SetWindowGeometry,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if req.height == 0 && req.width == 0 {
            // TODO: https://crbug.com/1329214
            return Ok(());
        }
        if req.height <= 0 || req.width <= 0 {
            return Err(XdgSurfaceError::NonPositiveWidthHeight);
        }
        let extents = Rect::new_sized_saturating(req.x, req.y, req.width, req.height);
        self.pending().geometry = Some(extents);
        Ok(())
    }

    fn ack_configure(&self, req: AckConfigure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let serial = self.surface.client.state.map_tree_serial32(req.serial);
        if let Some(last) = self.acked_serial.get()
            && serial <= last
        {
            return Err(XdgSurfaceError::InvalidSerial(serial.raw(), last.raw()));
        }
        self.acked_serial.set(Some(serial));
        self.surface.pending.borrow_mut().serial = Some(serial);
        Ok(())
    }
}

enum UpdateGeometryReason {
    FullscreenLive,
    FullscreenRender,
    Commit,
}

impl XdgSurface {
    fn update_effective_geometry(&self, reason: UpdateGeometryReason) {
        let ext = self.ext.get();
        let mut todo = SplitView::default();
        match reason {
            UpdateGeometryReason::FullscreenLive => {
                todo[LiveTL] = true;
                if let Some(ext) = &ext {
                    ext.clone()
                        .schedule_xdg_op(XdgSurfaceTransactionOp::UpdateGeometry);
                }
            }
            UpdateGeometryReason::FullscreenRender => {
                todo[RenderTL] = true;
            }
            UpdateGeometryReason::Commit => {
                todo[LiveTL] = true;
                todo[RenderTL] = true;
            }
        }
        if todo[LiveTL] {
            let v = self.calculate_effective_geometry(ext.as_ref(), LiveTL);
            if self.effective_geometry[LiveTL].replace(v) != v {
                self.update_surface_position();
            }
        }
        if todo[RenderTL] {
            let v = self.calculate_effective_geometry(ext.as_ref(), RenderTL);
            if self.effective_geometry[RenderTL].replace(v) != v
                && let Some(ext) = &ext
            {
                ext.geometry_changed();
            }
        }
    }

    fn calculate_effective_geometry(
        &self,
        ext: Option<&Rc<dyn XdgSurfaceExt>>,
        tl: TreeTimeline,
    ) -> Rect {
        let geometry = self
            .geometry
            .get()
            .unwrap_or_else(|| self.surface.extents.get());
        let mut effective_geometry = geometry;
        if let Some(ext) = &ext {
            effective_geometry = ext.effective_geometry(geometry, tl);
        }
        effective_geometry
    }

    fn update_extents(&self) {
        let old_extents = self.extents.get();
        let mut new_extents = self.surface.extents.get();
        if let Some(geometry) = self.geometry.get() {
            new_extents = new_extents.intersect(geometry);
        }
        self.extents.set(new_extents);
        if old_extents != new_extents {
            if self.geometry.is_none() {
                self.update_effective_geometry(UpdateGeometryReason::Commit);
            }
            if let Some(ext) = self.ext.get() {
                ext.extents_changed();
            }
        }
    }

    fn find_tree_at(&self, mut x: i32, mut y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        let geo = self.effective_geometry[LiveTL].get();
        (x, y) = geo.translate_inv(x, y);
        self.surface.find_tree_at_(x, y, tree)
    }

    fn update_popup_positions(&self) {
        let popups = self.popups.lock();
        for popup in popups.values() {
            popup.popup.update_absolute_position();
        }
    }

    fn set_visible(&self, visible: bool) {
        self.surface.set_visible(visible);
        for popup in self.popups.lock().values() {
            popup.popup.set_visible(visible);
        }
        if !visible {
            self.configure_data.ready();
        }
    }

    fn restack_popups(&self) {
        if self.popups.is_empty() {
            return;
        }
        for popup in self.popups.lock().values() {
            if self.surface.visible[RenderTL].get() {
                popup.popup.xdg.damage(RenderTL);
            }
            popup.display_link.borrow().restack();
            popup.popup.xdg.restack_popups();
        }
        self.surface.client.state.tree_changed();
    }
}

object_base! {
    self = XdgSurface;
    version = self.base.version;
}

impl Object for XdgSurface {
    fn break_loops(self: Rc<Self>) {
        self.destroyed.set(true);
        self.configure_data.ready();
        self.ext.take();
        self.popups.clear();
        self.workspace.set(None);
        self.workspace_type.set(None);
    }
}

dedicated_add_obj!(XdgSurface, XdgSurfaceId, xdg_surfaces);

impl SurfaceExt for XdgSurface {
    fn node_layer(&self) -> NodeLayerLink {
        let Some(ext) = self.ext.get() else {
            return NodeLayerLink::Display;
        };
        ext.node_layer()
    }

    fn commit_requested(
        self: Rc<Self>,
        _pending: &mut CachedBox<PendingState, BoxReset>,
    ) -> CommitAction {
        if let Some(ext) = self.ext.get() {
            ext.commit_requested();
        }
        CommitAction::ContinueCommit
    }

    fn before_apply_commit(
        self: Rc<Self>,
        pending: &mut PendingState,
        _serial: Option<TreeSerial>,
    ) -> Result<(), WlSurfaceError> {
        if let Some(geometry) = pending.xdg_surface.geometry.take() {
            let prev = self.geometry.replace(Some(geometry));
            if prev != Some(geometry) {
                self.update_effective_geometry(UpdateGeometryReason::Commit);
                self.update_extents();
            }
        }
        if let Some(ext) = self.ext.get() {
            ext.before_apply_commit(pending)?;
        }
        Ok(())
    }

    fn after_apply_commit(self: Rc<Self>) {
        match self.initial_commit_state.get() {
            InitialCommitState::Unmapped => {
                if let Some(ext) = self.ext.get() {
                    ext.initial_configure();
                    self.schedule_configure();
                    self.initial_commit_state.set(InitialCommitState::Sent);
                }
            }
            InitialCommitState::Sent => {
                if self.surface.buffer.is_some() {
                    self.initial_commit_state.set(InitialCommitState::Mapped);
                }
            }
            InitialCommitState::Mapped => {
                if self.surface.buffer.is_none() {
                    self.initial_commit_state.set(InitialCommitState::Unmapped);
                }
            }
        }
        if let Some(ext) = self.ext.get() {
            ext.post_commit();
        }
    }

    fn extents_changed(&self) {
        self.update_extents();
    }

    fn focus_node(&self) -> Option<Rc<dyn Node>> {
        self.ext.get()?.focus_node()
    }

    fn tray_item(self: Rc<Self>) -> Option<TrayItemId> {
        self.ext.get()?.tray_item()
    }

    fn configurable_data(&self) -> Option<&ConfigurableDataCore> {
        Some(self.configure_data.core())
    }

    fn workspace(&self) -> Option<Rc<WorkspaceNode>> {
        self.workspace.get()
    }
}

impl Configurable for XdgSurface {
    type T = XdgSurfaceConfigureData;

    fn data(&self) -> &ConfigurableData<Self::T> {
        &self.configure_data
    }

    fn configure_data(&self) -> Self::T {
        let Some(ext) = self.ext.get() else {
            return XdgSurfaceConfigureData::None;
        };
        ext.configure_data()
    }

    fn merge(first: &mut Self::T, mut second: Self::T) {
        if let XdgSurfaceConfigureData::Popup(old) = first
            && let XdgSurfaceConfigureData::Popup(new) = &mut second
            && new.repositioned.is_none()
        {
            new.repositioned = old.repositioned;
        }
        *first = second;
    }

    fn visible(&self) -> bool {
        self.surface.visible[LiveTL].get()
    }

    fn destroyed(&self) -> bool {
        self.destroyed.get() || self.ext.is_none()
    }

    fn surface(&self) -> &Rc<WlSurface> {
        &self.surface
    }

    fn flush(&self, serial: TreeSerial, data: Self::T) {
        if let Some(ext) = self.ext.get() {
            ext.send_configure(data);
        }
        self.send_configure(serial);
    }
}

#[derive(Debug, Error)]
pub enum XdgSurfaceError {
    #[error(
        "Surface {0} cannot be turned into a xdg_surface because it already has an attached xdg_surface"
    )]
    AlreadyAttached(WlSurfaceId),
    #[error(transparent)]
    XdgPopupError(#[from] XdgPopupError),
    #[error("Surface {} cannot be assigned the role {} because it already has the role {}", .id, .new.name(), .old.name())]
    IncompatibleRole {
        id: XdgSurfaceId,
        old: XdgSurfaceRole,
        new: XdgSurfaceRole,
    },
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Tried no set a non-positive width/height")]
    NonPositiveWidthHeight,
    #[error(
        "Cannot destroy xdg_surface {0} because it's associated xdg_toplevel/popup is not yet destroyed"
    )]
    RoleNotYetDestroyed(XdgSurfaceId),
    #[error("The surface still has popups attached")]
    PopupsNotYetDestroyed,
    #[error("The surface already has an assigned xdg_toplevel")]
    AlreadyConstructed,
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
    #[error("The serial {0} is not larger than the previously acked serial {1}")]
    InvalidSerial(u64, u64),
}
efrom!(XdgSurfaceError, WlSurfaceError);
efrom!(XdgSurfaceError, ClientError);

pub enum XdgSurfaceTransactionOp {
    UpdateGeometry,
    SetAbsoluteDesiredExtents(Rect),
}
