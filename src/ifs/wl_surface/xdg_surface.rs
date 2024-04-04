pub mod xdg_popup;
pub mod xdg_toplevel;

use {
    crate::{
        client::ClientError,
        ifs::{
            wl_surface::{
                xdg_surface::{
                    xdg_popup::{XdgPopup, XdgPopupError},
                    xdg_toplevel::{XdgToplevel, WM_CAPABILITIES_SINCE},
                },
                PendingState, SurfaceExt, SurfaceRole, WlSurface, WlSurfaceError,
            },
            xdg_wm_base::XdgWmBase,
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        tree::{FindTreeResult, FoundNode, OutputNode, WorkspaceNode},
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            numcell::NumCell,
            option_ext::OptionExt,
        },
        wire::{xdg_surface::*, WlSurfaceId, XdgPopupId, XdgSurfaceId},
    },
    std::{
        cell::{Cell, RefMut},
        fmt::Debug,
        rc::Rc,
    },
    thiserror::Error,
};

#[allow(dead_code)]
const NOT_CONSTRUCTED: u32 = 1;
const ALREADY_CONSTRUCTED: u32 = 2;
#[allow(dead_code)]
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

pub struct XdgSurface {
    id: XdgSurfaceId,
    base: Rc<XdgWmBase>,
    role: Cell<XdgSurfaceRole>,
    pub surface: Rc<WlSurface>,
    requested_serial: NumCell<u32>,
    acked_serial: Cell<Option<u32>>,
    geometry: Cell<Option<Rect>>,
    extents: Cell<Rect>,
    pub absolute_desired_extents: Cell<Rect>,
    ext: CloneCell<Option<Rc<dyn XdgSurfaceExt>>>,
    popups: CopyHashMap<XdgPopupId, Rc<XdgPopup>>,
    pub workspace: CloneCell<Option<Rc<WorkspaceNode>>>,
    pub tracker: Tracker<Self>,
}

#[derive(Default, Debug)]
pub struct PendingXdgSurfaceData {
    geometry: Option<Rect>,
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
    }
}

pub trait XdgSurfaceExt: Debug {
    fn initial_configure(self: Rc<Self>) -> Result<(), XdgSurfaceError> {
        Ok(())
    }

    fn post_commit(self: Rc<Self>) {
        // nothing
    }

    fn extents_changed(&self) {
        // nothing
    }
}

impl XdgSurface {
    pub fn new(wm_base: &Rc<XdgWmBase>, id: XdgSurfaceId, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
            base: wm_base.clone(),
            role: Cell::new(XdgSurfaceRole::None),
            surface: surface.clone(),
            requested_serial: NumCell::new(1),
            acked_serial: Cell::new(None),
            geometry: Cell::new(None),
            extents: Cell::new(Default::default()),
            absolute_desired_extents: Cell::new(Default::default()),
            ext: Default::default(),
            popups: Default::default(),
            workspace: Default::default(),
            tracker: Default::default(),
        }
    }

    fn set_absolute_desired_extents(&self, ext: &Rect) {
        let prev = self.absolute_desired_extents.replace(*ext);
        if ext.position() != prev.position() {
            let (mut x1, mut y1) = (ext.x1(), ext.y1());
            if let Some(geo) = self.geometry.get() {
                x1 -= geo.x1();
                y1 -= geo.y1();
            }
            self.surface.set_absolute_position(x1, y1);
            self.update_popup_positions();
        }
    }

    fn set_workspace(&self, ws: &Rc<WorkspaceNode>) {
        self.workspace.set(Some(ws.clone()));
        self.surface.set_output(&ws.output.get());
        let pu = self.popups.lock();
        for pu in pu.values() {
            pu.xdg.set_workspace(ws);
        }
    }

    pub fn set_output(&self, output: &Rc<OutputNode>) {
        self.surface.set_output(output);
        let pu = self.popups.lock();
        for pu in pu.values() {
            pu.xdg.set_output(output);
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
                })
            }
        }
        self.role.set(role);
        Ok(())
    }

    fn destroy_node(&self) {
        self.workspace.set(None);
        self.surface.destroy_node();
        let popups = self.popups.lock();
        for popup in popups.values() {
            popup.destroy_node();
        }
    }

    fn detach_node(&self) {
        self.workspace.set(None);
        self.surface.detach_node(false);
        let popups = self.popups.lock();
        for popup in popups.values() {
            popup.detach_node();
        }
    }

    pub fn geometry(&self) -> Option<Rect> {
        self.geometry.get()
    }

    pub fn do_send_configure(&self) {
        let serial = self.requested_serial.fetch_add(1) + 1;
        self.send_configure(serial);
    }

    pub fn send_configure(&self, serial: u32) {
        self.surface.client.event(Configure {
            self_id: self.id,
            serial,
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

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgSurfaceError> {
        let _req: Destroy = self.surface.client.parse(self, parser)?;
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

    fn get_toplevel(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), XdgSurfaceError> {
        let req: GetToplevel = self.surface.client.parse(&**self, parser)?;
        self.set_role(XdgSurfaceRole::XdgToplevel)?;
        if self.ext.is_some() {
            self.surface.client.protocol_error(
                &**self,
                ALREADY_CONSTRUCTED,
                &format!(
                    "wl_surface {} already has an assigned xdg_toplevel",
                    self.surface.id
                ),
            );
            return Err(XdgSurfaceError::AlreadyConstructed);
        }
        let toplevel = Rc::new(XdgToplevel::new(req.id, self));
        track!(self.surface.client, toplevel);
        self.surface.client.add_client_obj(&toplevel)?;
        self.ext.set(Some(toplevel.clone()));
        if self.base.version >= WM_CAPABILITIES_SINCE {
            toplevel.send_wm_capabilities();
        }
        self.surface.set_toplevel(Some(toplevel));
        Ok(())
    }

    fn get_popup(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), XdgSurfaceError> {
        let req: GetPopup = self.surface.client.parse(&**self, parser)?;
        self.set_role(XdgSurfaceRole::XdgPopup)?;
        let mut parent = None;
        if req.parent.is_some() {
            parent = Some(self.surface.client.lookup(req.parent)?);
        }
        let positioner = self.surface.client.lookup(req.positioner)?;
        if self.ext.is_some() {
            self.surface.client.protocol_error(
                &**self,
                ALREADY_CONSTRUCTED,
                &format!(
                    "wl_surface {} already has an assigned xdg_popup",
                    self.surface.id
                ),
            );
            return Err(XdgSurfaceError::AlreadyConstructed);
        }
        let popup = Rc::new(XdgPopup::new(req.id, self, parent.as_ref(), &positioner)?);
        track!(self.surface.client, popup);
        self.surface.client.add_client_obj(&popup)?;
        if let Some(parent) = &parent {
            parent.popups.set(req.id, popup.clone());
        }
        self.ext.set(Some(popup));
        Ok(())
    }

    fn pending(&self) -> RefMut<Box<PendingXdgSurfaceData>> {
        RefMut::map(self.surface.pending.borrow_mut(), |p| {
            p.xdg_surface.get_or_insert_default_ext()
        })
    }

    fn set_window_geometry(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgSurfaceError> {
        let req: SetWindowGeometry = self.surface.client.parse(self, parser)?;
        if req.height == 0 && req.width == 0 {
            // TODO: https://crbug.com/1329214
            return Ok(());
        }
        if req.height <= 0 || req.width <= 0 {
            return Err(XdgSurfaceError::NonPositiveWidthHeight);
        }
        let extents = Rect::new_sized(req.x, req.y, req.width, req.height).unwrap();
        self.pending().geometry = Some(extents);
        Ok(())
    }

    fn ack_configure(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgSurfaceError> {
        let req: AckConfigure = self.surface.client.parse(self, parser)?;
        if self.requested_serial.get() == req.serial {
            self.acked_serial.set(Some(req.serial));
        }
        Ok(())
    }

    fn update_extents(&self) {
        let old_extents = self.extents.get();
        let mut new_extents = self.surface.extents.get();
        if let Some(geometry) = self.geometry.get() {
            new_extents = new_extents.intersect(geometry);
        }
        self.extents.set(new_extents);
        if old_extents != new_extents {
            if let Some(ext) = self.ext.get() {
                ext.extents_changed();
            }
        }
    }

    fn find_tree_at(&self, mut x: i32, mut y: i32, tree: &mut Vec<FoundNode>) -> FindTreeResult {
        if let Some(geo) = self.geometry.get() {
            let (xt, yt) = geo.translate_inv(x, y);
            x = xt;
            y = yt;
        }
        self.surface.find_tree_at_(x, y, tree)
    }

    fn update_popup_positions(&self) {
        let popups = self.popups.lock();
        for popup in popups.values() {
            popup.update_absolute_position();
        }
    }

    fn set_visible(&self, visible: bool) {
        self.surface.set_visible(visible);
        for popup in self.popups.lock().values() {
            popup.set_visible(visible);
        }
    }

    fn restack_popups(&self) {
        for popup in self.popups.lock().values() {
            popup.restack();
        }
    }
}

object_base! {
    self = XdgSurface;

    DESTROY => destroy,
    GET_TOPLEVEL => get_toplevel,
    GET_POPUP => get_popup,
    SET_WINDOW_GEOMETRY => set_window_geometry,
    ACK_CONFIGURE => ack_configure,
}

impl Object for XdgSurface {
    fn break_loops(&self) {
        self.ext.take();
        self.popups.clear();
        self.workspace.set(None);
    }
}

dedicated_add_obj!(XdgSurface, XdgSurfaceId, xdg_surfaces);

impl SurfaceExt for XdgSurface {
    fn before_apply_commit(
        self: Rc<Self>,
        pending: &mut PendingState,
    ) -> Result<(), WlSurfaceError> {
        {
            let ase = self.acked_serial.get();
            let rse = self.requested_serial.get();
            if ase != Some(rse) {
                if ase.is_none() {
                    if let Some(ext) = self.ext.get() {
                        ext.initial_configure()?;
                    }
                    self.send_configure(rse);
                }
            }
        }
        if let Some(pending) = &mut pending.xdg_surface {
            if let Some(geometry) = pending.geometry.take() {
                self.geometry.set(Some(geometry));
                self.update_extents();
            }
        }
        Ok(())
    }

    fn after_apply_commit(self: Rc<Self>) {
        if let Some(ext) = self.ext.get() {
            ext.post_commit();
        }
    }

    fn extents_changed(&self) {
        self.update_extents();
    }
}

#[derive(Debug, Error)]
pub enum XdgSurfaceError {
    #[error("Surface {0} cannot be turned into a xdg_surface because it already has an attached xdg_surface")]
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
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("Tried no set a non-positive width/height")]
    NonPositiveWidthHeight,
    #[error("Cannot destroy xdg_surface {0} because it's associated xdg_toplevel/popup is not yet destroyed")]
    RoleNotYetDestroyed(XdgSurfaceId),
    #[error("The surface still has popups attached")]
    PopupsNotYetDestroyed,
    #[error("The surface already has an assigned xdg_toplevel")]
    AlreadyConstructed,
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
}
efrom!(XdgSurfaceError, WlSurfaceError);
efrom!(XdgSurfaceError, ClientError);
efrom!(XdgSurfaceError, MsgParserError);
