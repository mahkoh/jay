mod types;
pub mod xdg_popup;
pub mod xdg_toplevel;

use crate::client::DynEventFormatter;
use crate::ifs::wl_surface::xdg_surface::xdg_popup::{XdgPopup, XdgPopupId};
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::wl_surface::{CommitAction, CommitContext, SurfaceExt, SurfaceRole, WlSurface};
use crate::ifs::xdg_wm_base::XdgWmBaseObj;
use crate::object::{Interface, Object, ObjectId};
use crate::rect::Rect;
use crate::tree::Node;
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::NumCell;
use std::cell::Cell;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const GET_TOPLEVEL: u32 = 1;
const GET_POPUP: u32 = 2;
const SET_WINDOW_GEOMETRY: u32 = 3;
const ACK_CONFIGURE: u32 = 4;

const CONFIGURE: u32 = 0;

#[allow(dead_code)]
const NOT_CONSTRUCTED: u32 = 1;
const ALREADY_CONSTRUCTED: u32 = 2;
#[allow(dead_code)]
const UNCONFIGURED_BUFFER: u32 = 3;

id!(XdgSurfaceId);

pub struct XdgSurface {
    id: XdgSurfaceId,
    base: Rc<XdgWmBaseObj>,
    pub surface: Rc<WlSurface>,
    requested_serial: NumCell<u32>,
    acked_serial: Cell<Option<u32>>,
    geometry: Cell<Option<Rect>>,
    extents: Cell<Rect>,
    ext: CloneCell<Option<Rc<dyn XdgSurfaceExt>>>,
    popups: CopyHashMap<XdgPopupId, Rc<XdgPopup>>,
    pending: PendingXdgSurfaceData,
}

#[derive(Default)]
struct PendingXdgSurfaceData {
    geometry: Cell<Option<Rect>>,
}

trait XdgSurfaceExt {
    fn initial_configure(self: Rc<Self>) {
        // nothing
    }

    fn pre_commit(self: Rc<Self>) {
        // nothing
    }

    fn post_commit(self: Rc<Self>) {
        // nothing
    }

    fn into_node(self: Rc<Self>) -> Option<Rc<dyn Node>> {
        None
    }

    fn extents_changed(&self) {
        // nothing
    }
}

impl XdgSurface {
    pub fn new(wm_base: &Rc<XdgWmBaseObj>, id: XdgSurfaceId, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
            base: wm_base.clone(),
            surface: surface.clone(),
            requested_serial: NumCell::new(0),
            acked_serial: Cell::new(None),
            geometry: Cell::new(None),
            extents: Cell::new(Default::default()),
            ext: Default::default(),
            popups: Default::default(),
            pending: Default::default(),
        }
    }

    pub fn geometry(&self) -> Option<Rect> {
        self.geometry.get()
    }

    pub fn send_configure(self: &Rc<Self>) {
        let serial = self.requested_serial.fetch_add(1) + 1;
        self.surface.client.event(self.configure(serial));
    }

    pub fn configure(self: &Rc<Self>, serial: u32) -> DynEventFormatter {
        Box::new(Configure {
            obj: self.clone(),
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

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.surface.client.parse(self, parser)?;
        if self.ext.get().is_some() {
            return Err(DestroyError::RoleNotYetDestroyed(self.id));
        }
        {
            let children = self.popups.lock();
            for child in children.values() {
                child.parent.set(None);
            }
        }
        self.surface.unset_ext();
        self.base.surfaces.remove(&self.id);
        self.surface.client.remove_obj(self)?;
        Ok(())
    }

    fn get_toplevel(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetToplevelError> {
        let req: GetToplevel = self.surface.client.parse(&**self, parser)?;
        self.surface.set_role(SurfaceRole::XdgToplevel)?;
        if self.ext.get().is_some() {
            self.surface.client.protocol_error(
                &**self,
                ALREADY_CONSTRUCTED,
                format!(
                    "wl_surface {} already has an assigned xdg_toplevel",
                    self.surface.id
                ),
            );
            return Err(GetToplevelError::AlreadyConstructed);
        }
        let toplevel = Rc::new(XdgToplevel::new(req.id, self));
        self.surface.client.add_client_obj(&toplevel)?;
        self.ext.set(Some(toplevel));
        Ok(())
    }

    fn get_popup(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetPopupError> {
        let req: GetPopup = self.surface.client.parse(&**self, parser)?;
        self.surface.set_role(SurfaceRole::XdgPopup)?;
        let mut parent = None;
        if req.parent.is_some() {
            parent = Some(self.surface.client.get_xdg_surface(req.parent)?);
        }
        if self.ext.get().is_some() {
            self.surface.client.protocol_error(
                &**self,
                ALREADY_CONSTRUCTED,
                format!(
                    "wl_surface {} already has an assigned xdg_popup",
                    self.surface.id
                ),
            );
            return Err(GetPopupError::AlreadyConstructed);
        }
        let popup = Rc::new(XdgPopup::new(req.id, self, parent.as_ref()));
        self.surface.client.add_client_obj(&popup)?;
        if let Some(parent) = &parent {
            parent.popups.set(req.id, popup.clone());
        }
        self.ext.set(Some(popup));
        Ok(())
    }

    fn set_window_geometry(&self, parser: MsgParser<'_, '_>) -> Result<(), SetWindowGeometryError> {
        let req: SetWindowGeometry = self.surface.client.parse(self, parser)?;
        if req.height <= 0 || req.width <= 0 {
            return Err(SetWindowGeometryError::NonPositiveWidthHeight);
        }
        let extents = Rect::new_sized(req.x, req.y, req.width, req.height).unwrap();
        self.pending.geometry.set(Some(extents));
        Ok(())
    }

    fn ack_configure(&self, parser: MsgParser<'_, '_>) -> Result<(), AckConfigureError> {
        let req: AckConfigure = self.surface.client.parse(self, parser)?;
        if self.requested_serial.get() == req.serial {
            self.acked_serial.set(Some(req.serial));
        }
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), XdgSurfaceError> {
        match request {
            DESTROY => self.destroy(parser)?,
            GET_TOPLEVEL => self.get_toplevel(parser)?,
            GET_POPUP => self.get_popup(parser)?,
            SET_WINDOW_GEOMETRY => self.set_window_geometry(parser)?,
            ACK_CONFIGURE => self.ack_configure(parser)?,
            _ => unreachable!(),
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
}

handle_request!(XdgSurface);

impl Object for XdgSurface {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::XdgSurface
    }

    fn num_requests(&self) -> u32 {
        ACK_CONFIGURE + 1
    }
}

impl SurfaceExt for XdgSurface {
    fn pre_commit(self: Rc<Self>, _ctx: CommitContext) -> CommitAction {
        {
            let ase = self.acked_serial.get();
            let rse = self.requested_serial.get();
            if ase != Some(rse) {
                if ase.is_none() {
                    if let Some(ext) = self.ext.get() {
                        ext.initial_configure();
                    }
                    self.surface.client.event(self.configure(rse));
                }
                // return CommitAction::AbortCommit;
            }
        }
        if let Some(geometry) = self.pending.geometry.take() {
            self.geometry.set(Some(geometry));
            self.update_extents();
        }
        CommitAction::ContinueCommit
    }

    fn post_commit(&self) {
        if let Some(ext) = self.ext.get() {
            ext.post_commit();
        }
    }

    fn extents_changed(&self) {
        self.update_extents();
    }

    fn into_node(self: Rc<Self>) -> Option<Rc<dyn Node>> {
        match self.ext.get() {
            Some(e) => e.into_node(),
            _ => None,
        }
    }
}
