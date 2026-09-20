use crate::client::Client;
use crate::configurable::Configurable;
use crate::configurable::ConfigurableData;
use crate::configurable::ConfigurableDataCore;
use crate::configurable::ConfigurableExt;
use crate::ifs::wl_surface::SurfaceExt;
use crate::ifs::wl_surface::SurfaceRole;
use crate::ifs::wl_surface::WlSurface;
use crate::ifs::wl_surface::icon_surface::IconSurfaceOwner;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_v1::private::ConfData;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_v1::private::IconOp;
use crate::ifs::wl_surface::surface_render_cache::SurfaceRenderCache;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::rect::Rect;
use crate::renderer::renderer_base::RendererBase;
use crate::transactions::EnabledSurfaceTransactions;
use crate::transactions::TransactionData;
use crate::transactions::Transactionable;
use crate::transactions::TransactionableExt;
use crate::tree::NodeBase;
use crate::tree::NodeId;
use crate::tree::NodeLayerLink;
use crate::tree::TreeSerial;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::WorkspaceNode;
use crate::utils::cell_ext::CellExt;
use crate::utils::clonecell::CloneCell;
use crate::utils::markers::JayClone;
use crate::utils::numcell::NumCell;
use crate::wire::JayIconSurfaceV1Id;
use crate::wire::ObjectId;
use crate::wire::jay_icon_surface_v1::*;
use jay_proc::Object;
use std::cell::Cell;
use std::rc::Rc;
use thiserror::Error;

pub struct IconSurface {
    surface: Rc<JayIconSurfaceV1>,
}

impl IconSurface {
    pub fn disown(&self) {
        self.surface.owner.take();
    }

    pub fn set_visible(&self, visible: bool) {
        let s = &self.surface;
        s.surface.set_visible(visible);
        let et = &s.enabled_surface_transactions;
        if visible {
            if et.is_none() {
                et.set(Some(s.surface.enable_transactions()));
            }
        } else {
            et.take();
            s.configurable_data.ready();
        }
    }

    pub fn set_position(&self, x: i32, y: i32) {
        let s = &self.surface;
        let pos = Some((x, y));
        if s.position.replace(pos) != pos {
            s.surface.set_absolute_position(x, y);
        }
    }

    pub fn set_size(&self, mut width: i32, mut height: i32) {
        width = width.max(1);
        height = height.max(1);
        let size = (width, height);
        let s = &self.surface;
        s.size.set(size);
        s.add_transaction_op(IconOp::SetSize(size));
        s.schedule_configure();
    }

    pub fn set_workspace(&self, v: &Rc<WorkspaceNode>) {
        let s = &self.surface;
        s.workspace.set(Some(v.clone()));
        s.surface.set_workspace(v);
    }

    pub fn set_grayscale(&self, grayscale: bool) {
        let s = &self.surface;
        s.add_transaction_op(IconOp::SetGrayscale(grayscale));
    }

    pub fn render(&self, renderer: &mut RendererBase<'_>, x: i32, y: i32, bounds: Option<&Rect>) {
        let s = &self.surface;
        if !s.surface.node_visible(RenderTL) || s.surface.buffer.is_none() {
            return;
        }
        s.cache.render(renderer, x, y, s.grayscale.get(), bounds);
    }

    #[cfg(feature = "it")]
    pub fn grayscale(&self) -> bool {
        let s = &self.surface;
        s.grayscale.get()
    }
}

impl Clone for IconSurface {
    fn clone(&self) -> Self {
        self.surface.users.add_fetch(1);
        Self {
            surface: self.surface.clone(),
        }
    }
}

unsafe impl JayClone for IconSurface {}

impl Drop for IconSurface {
    fn drop(&mut self) {
        let s = &self.surface;
        if s.users.sub_fetch(1) == 0 {
            self.disown();
            s.finished();
        }
    }
}

#[derive(Object)]
#[break_loops]
pub struct JayIconSurfaceV1 {
    id: JayIconSurfaceV1Id,
    cache: Rc<SurfaceRenderCache>,
    client: Rc<Client>,
    configurable_data: ConfigurableData<ConfData>,
    enabled_surface_transactions: Cell<Option<EnabledSurfaceTransactions>>,
    finished: Cell<bool>,
    grayscale: Cell<bool>,
    last_serial: Cell<Option<u64>>,
    owner: CloneCell<Option<Rc<dyn IconSurfaceOwner>>>,
    node_id: NodeId,
    position: Cell<Option<(i32, i32)>>,
    size: Cell<(i32, i32)>,
    surface: Rc<WlSurface>,
    tracker: Tracker<Self>,
    transaction_data: TransactionData<IconOp>,
    users: NumCell<u64>,
    version: Version,
    workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
}

mod private {
    pub struct ConfData {
        pub width: i32,
        pub height: i32,
    }

    pub enum IconOp {
        SetGrayscale(bool),
        SetSize((i32, i32)),
    }
}

impl IconSurface {
    pub(super) fn new(
        client: &Rc<Client>,
        id: JayIconSurfaceV1Id,
        version: Version,
        surface: &Rc<WlSurface>,
        owner: Rc<dyn IconSurfaceOwner>,
        node_id: NodeId,
    ) -> Self {
        let state = &client.state;
        let surface = Rc::new(JayIconSurfaceV1 {
            id,
            cache: SurfaceRenderCache::new(surface),
            client: client.clone(),
            configurable_data: ConfigurableData::new(state),
            enabled_surface_transactions: Default::default(),
            finished: Default::default(),
            grayscale: Default::default(),
            last_serial: Default::default(),
            owner: CloneCell::new(Some(owner)),
            node_id,
            position: Default::default(),
            size: Cell::new((1, 1)),
            surface: surface.clone(),
            tracker: Default::default(),
            transaction_data: TransactionData::new(&state.tree),
            users: NumCell::new(1),
            version,
            workspace: Default::default(),
        });
        track!(client, surface);
        client.add_server_obj(&surface);
        let _ = surface.surface.set_ext(SurfaceRole::Icon, surface.clone());
        IconSurface { surface }
    }
}

impl JayIconSurfaceV1 {
    fn detach(&self) {
        if let Some(owner) = self.owner.take() {
            owner.destroyed(self.node_id);
        }
    }

    pub fn finished(&self) {
        self.surface.destroy_node();
        self.enabled_surface_transactions.take();
        self.configurable_data.ready();
        if !self.finished.replace(true) {
            self.client.event(Finished { self_id: self.id });
        }
    }

    fn send_configure_size(&self, width: i32, height: i32) {
        self.client.event(ConfigureSize {
            self_id: self.id,
            width,
            height,
        });
    }

    fn send_configure(&self, serial: TreeSerial) {
        self.client.event(Configure {
            self_id: self.id,
            serial: serial.raw(),
        });
    }
}

impl SurfaceExt for JayIconSurfaceV1 {
    fn object_id(&self) -> Option<ObjectId> {
        Some(self.id.into())
    }

    fn node_layer(&self) -> NodeLayerLink {
        NodeLayerLink::Display
    }

    fn configurable_data(&self) -> Option<&ConfigurableDataCore> {
        Some(self.configurable_data.core())
    }

    fn workspace(&self) -> Option<Rc<WorkspaceNode>> {
        self.workspace.get()
    }

    fn unmap(self: Rc<Self>) {
        // nothing
    }
}

impl Configurable for JayIconSurfaceV1 {
    type T = ConfData;

    fn data(&self) -> &ConfigurableData<Self::T> {
        &self.configurable_data
    }

    fn configure_data(&self) -> Self::T {
        let (width, height) = self.size.get();
        ConfData { width, height }
    }

    fn merge(first: &mut Self::T, second: Self::T) {
        *first = second;
    }

    fn visible(&self) -> bool {
        self.surface.node_visible(LiveTL)
    }

    fn destroyed(&self) -> bool {
        self.finished.get()
    }

    fn surface(&self) -> &Rc<WlSurface> {
        &self.surface
    }

    fn flush(&self, serial: TreeSerial, data: Self::T) {
        self.send_configure_size(data.width, data.height);
        self.send_configure(serial);
    }
}

impl Transactionable for JayIconSurfaceV1 {
    type T = IconOp;

    fn data(&self) -> &TransactionData<Self::T> {
        &self.transaction_data
    }

    fn apply(self: &Rc<Self>, op: Self::T) {
        match op {
            IconOp::SetSize((w, h)) => {
                self.cache.set_size(w, h);
            }
            IconOp::SetGrayscale(v) => {
                self.grayscale.set(v);
            }
        }
    }
}

impl JayIconSurfaceV1RequestHandler for JayIconSurfaceV1 {
    type Error = JayIconSurfaceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.finished.set(true);
        self.enabled_surface_transactions.take();
        self.configurable_data.ready();
        self.surface.destroy_node();
        self.detach();
        self.surface.unset_ext();
        self.client.remove_obj(self);
        Ok(())
    }

    fn ack_configure(&self, req: AckConfigure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(last) = self.last_serial.replace(Some(req.serial))
            && last >= req.serial
        {
            return Err(JayIconSurfaceV1Error::NonMonotonicSerial);
        }
        let Some(serial) = self.client.state.validate_tree_serial(req.serial) else {
            return Err(JayIconSurfaceV1Error::InvalidSerial);
        };
        self.surface.pending.borrow_mut().serial = Some(serial);
        Ok(())
    }
}

impl BreakLoops for JayIconSurfaceV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

#[derive(Debug, Error)]
pub enum JayIconSurfaceV1Error {
    #[error("The serial is invalid")]
    InvalidSerial,
    #[error("The serial is not strictly larger than the previous serial")]
    NonMonotonicSerial,
}
