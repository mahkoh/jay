mod types;
pub mod wl_subsurface;
pub mod xdg_surface;

use crate::client::{AddObj, Client, RequestParser};
use crate::ifs::wl_buffer::WlBuffer;
use crate::ifs::wl_callback::WlCallback;
use crate::ifs::wl_surface::wl_subsurface::WlSubsurface;
use crate::ifs::wl_surface::xdg_surface::xdg_popup::XdgPopup;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::object::{Interface, Object, ObjectId};
use crate::pixman::Region;
use crate::tree::{NodeBase, NodeCommon, ToplevelNode};
use crate::utils::buffd::{MsgParser, MsgParserError};
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::{LinkedList, Node as LinkNode};
use ahash::AHashMap;
use std::cell::{Cell, RefCell};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const ATTACH: u32 = 1;
const DAMAGE: u32 = 2;
const FRAME: u32 = 3;
const SET_OPAQUE_REGION: u32 = 4;
const SET_INPUT_REGION: u32 = 5;
const COMMIT: u32 = 6;
const SET_BUFFER_TRANSFORM: u32 = 7;
const SET_BUFFER_SCALE: u32 = 8;
const DAMAGE_BUFFER: u32 = 9;

#[allow(dead_code)]
const ENTER: u32 = 0;
#[allow(dead_code)]
const LEAVE: u32 = 1;

#[allow(dead_code)]
const INVALID_SCALE: u32 = 0;
#[allow(dead_code)]
const INVALID_TRANSFORM: u32 = 1;
#[allow(dead_code)]
const INVALID_SIZE: u32 = 2;

id!(WlSurfaceId);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SurfaceRole {
    None,
    Subsurface,
    XdgSurface,
}

impl SurfaceRole {
    fn name(self) -> &'static str {
        match self {
            SurfaceRole::None => "none",
            SurfaceRole::Subsurface => "subsurface",
            SurfaceRole::XdgSurface => "xdg_surface",
        }
    }
}

pub struct WlSurface {
    pub id: WlSurfaceId,
    pub client: Rc<Client>,
    role: Cell<SurfaceRole>,
    pending: PendingState,
    input_region: Cell<Option<Region>>,
    opaque_region: Cell<Option<Region>>,
    pub extents: Cell<SurfaceExtents>,
    pub effective_extents: Cell<SurfaceExtents>,
    pub buffer: CloneCell<Option<Rc<WlBuffer>>>,
    pub children: RefCell<Option<Box<ParentData>>>,
    role_data: RefCell<RoleData>,
    pub frame_requests: RefCell<Vec<Rc<WlCallback>>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct SurfaceExtents {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

enum RoleData {
    None,
    Subsurface(Box<SubsurfaceData>),
    XdgSurface(Box<XdgSurfaceData>),
}

impl RoleData {
    fn is_some(&self) -> bool {
        !matches!(self, RoleData::None)
    }
}

#[derive(Default)]
struct PendingState {
    opaque_region: Cell<Option<Region>>,
    input_region: Cell<Option<Region>>,
    frame_request: RefCell<Vec<Rc<WlCallback>>>,
}

struct XdgSurfaceData {
    xdg_surface: Rc<XdgSurface>,
    requested_serial: u32,
    acked_serial: Option<u32>,
    role: XdgSurfaceRole,
    extents: Option<SurfaceExtents>,
    role_data: XdgSurfaceRoleData,
    popups: CopyHashMap<WlSurfaceId, Rc<XdgPopup>>,
    pending: PendingXdgSurfaceData,
}

#[derive(Default)]
struct PendingXdgSurfaceData {
    extents: Cell<Option<SurfaceExtents>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum XdgSurfaceRole {
    None,
    Popup,
    Toplevel,
}

impl XdgSurfaceRole {
    fn is_compatible(self, role: XdgSurfaceRole) -> bool {
        self == XdgSurfaceRole::None || self == role
    }
}

enum XdgSurfaceRoleData {
    None,
    Popup(XdgPopupData),
    Toplevel(XdgToplevelData),
}

impl XdgSurfaceRoleData {
    fn is_some(&self) -> bool {
        !matches!(self, XdgSurfaceRoleData::None)
    }
}

struct XdgPopupData {
    _popup: Rc<XdgPopup>,
    parent: Option<Rc<XdgSurface>>,
}

struct XdgToplevelData {
    toplevel: Rc<XdgToplevel>,
    node: Option<ToplevelNodeHolder>,
}

struct ToplevelNodeHolder {
    node: Rc<ToplevelNode>,
}

impl Drop for ToplevelNodeHolder {
    fn drop(&mut self) {
        mem::take(&mut *self.node.common.floating_outputs.borrow_mut());
    }
}

struct SubsurfaceData {
    subsurface: Rc<WlSubsurface>,
    x: i32,
    y: i32,
    sync_requested: bool,
    sync_ancestor: bool,
    node: LinkNode<StackElement>,
    depth: u32,
    pending: PendingSubsurfaceData,
}

#[derive(Default)]
struct PendingSubsurfaceData {
    node: Option<LinkNode<StackElement>>,
    position: Option<(i32, i32)>,
}

#[derive(Default)]
pub struct ParentData {
    subsurfaces: AHashMap<WlSurfaceId, Rc<WlSurface>>,
    pub below: LinkedList<StackElement>,
    pub above: LinkedList<StackElement>,
}

pub struct StackElement {
    pending: Cell<bool>,
    pub surface: Rc<WlSurface>,
}

impl WlSurface {
    pub fn new(id: WlSurfaceId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            role: Cell::new(SurfaceRole::None),
            pending: Default::default(),
            input_region: Cell::new(None),
            opaque_region: Cell::new(None),
            extents: Default::default(),
            effective_extents: Default::default(),
            buffer: CloneCell::new(None),
            children: Default::default(),
            role_data: RefCell::new(RoleData::None),
            frame_requests: RefCell::new(vec![]),
        }
    }

    pub fn subsurface_position(&self) -> Option<(i32, i32)> {
        let rd = self.role_data.borrow();
        match rd.deref() {
            RoleData::Subsurface(ss) => Some((ss.x, ss.y)),
            _ => None,
        }
    }

    fn calculate_extents(&self) {
        {
            let mut extents = SurfaceExtents::default();
            if let Some(b) = self.buffer.get() {
                extents.x2 = b.width as i32;
                extents.y2 = b.height as i32;
            }
            let children = self.children.borrow();
            if let Some(children) = &*children {
                for surface in children.subsurfaces.values() {
                    let rd = surface.role_data.borrow();
                    if let RoleData::Subsurface(ss) = &*rd {
                        let ss_extents = surface.extents.get();
                        extents.x1 = extents.x1.min(ss_extents.x1 + ss.x);
                        extents.y1 = extents.y1.min(ss_extents.y1 + ss.y);
                        extents.x2 = extents.x2.max(ss_extents.x2 + ss.x);
                        extents.y2 = extents.y2.max(ss_extents.y2 + ss.y);
                    }
                }
            }
            self.extents.set(extents);
        }
        let parent = {
            let rd = self.role_data.borrow();
            match &*rd {
                RoleData::Subsurface(ss) => ss.subsurface.parent.clone(),
                _ => return,
            }
        };
        parent.calculate_extents();
    }

    pub fn get_root(self: &Rc<Self>) -> Rc<WlSurface> {
        let mut root = self.clone();
        loop {
            let tmp = root;
            let data = tmp.role_data.borrow();
            match &*data {
                RoleData::Subsurface(d) => root = d.subsurface.parent.clone(),
                _ => {
                    drop(data);
                    return tmp;
                }
            }
        }
    }

    fn parse<'a, T: RequestParser<'a>>(
        &self,
        parser: MsgParser<'_, 'a>,
    ) -> Result<T, MsgParserError> {
        self.client.parse(self, parser)
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.parse(parser)?;
        if self.role_data.borrow().is_some() {
            return Err(DestroyError::ReloObjectStillExists);
        }
        {
            let mut children = self.children.borrow_mut();
            if let Some(children) = &mut *children {
                for surface in children.subsurfaces.values() {
                    *surface.role_data.borrow_mut() = RoleData::None;
                }
            }
            *children = None;
        }
        {
            let buffer = self.buffer.get();
            if let Some(buffer) = &buffer {
                buffer.surfaces.remove(&self.id);
            }
        }
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn attach(&self, parser: MsgParser<'_, '_>) -> Result<(), AttachError> {
        let req: Attach = self.parse(parser)?;
        {
            if let Some(buffer) = self.buffer.take() {
                self.client.event(buffer.release()).await?;
                buffer.surfaces.remove(&self.id);
            }
            let mut rd = self.role_data.borrow_mut();
            if req.buffer.is_some() {
                self.buffer.set(Some(self.client.get_buffer(req.buffer)?));
                if let RoleData::XdgSurface(xdg) = &mut *rd {
                    if let XdgSurfaceRoleData::Toplevel(td) = &mut xdg.role_data {
                        if td.node.is_none() {
                            let outputs = self.client.state.root.outputs.lock();
                            if let Some(output) = outputs.values().next() {
                                let node = Rc::new(ToplevelNode {
                                    common: NodeCommon {
                                        extents: Cell::new(Default::default()),
                                        id: self.client.state.node_ids.next(),
                                        parent: Some(output.clone()),
                                        floating_outputs: RefCell::new(Default::default()),
                                    },
                                    surface: td.toplevel.clone(),
                                });
                                td.node = Some(ToplevelNodeHolder { node: node.clone() });
                                let link = output.floating.add_last(node.clone());
                                node.common
                                    .floating_outputs
                                    .borrow_mut()
                                    .insert(output.id(), link);
                            }
                        }
                    }
                }
            } else {
                self.buffer.set(None);
                if let RoleData::XdgSurface(xdg) = &mut *rd {
                    if let XdgSurfaceRoleData::Toplevel(td) = &mut xdg.role_data {
                        td.node = None;
                    }
                }
            }
        }
        Ok(())
    }

    async fn damage(&self, parser: MsgParser<'_, '_>) -> Result<(), DamageError> {
        let _req: Damage = self.parse(parser)?;
        Ok(())
    }

    async fn frame(&self, parser: MsgParser<'_, '_>) -> Result<(), FrameError> {
        let req: Frame = self.parse(parser)?;
        let cb = Rc::new(WlCallback::new(req.callback));
        self.client.add_client_obj(&cb)?;
        self.pending.frame_request.borrow_mut().push(cb);
        Ok(())
    }

    async fn set_opaque_region(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), SetOpaqueRegionError> {
        let region: SetOpaqueRegion = self.parse(parser)?;
        let region = self.client.get_region(region.region)?;
        self.pending.opaque_region.set(Some(region.region()));
        Ok(())
    }

    async fn set_input_region(&self, parser: MsgParser<'_, '_>) -> Result<(), SetInputRegionError> {
        let req: SetInputRegion = self.parse(parser)?;
        let region = self.client.get_region(req.region)?;
        self.pending.input_region.set(Some(region.region()));
        Ok(())
    }

    fn do_commit(&self) {
        let mut xdg_extents = None;
        let mut td_node = None;
        {
            let mut rd = self.role_data.borrow_mut();
            match &mut *rd {
                RoleData::None => {}
                RoleData::Subsurface(ss) => {
                    if let Some(v) = ss.pending.node.take() {
                        v.pending.set(false);
                        ss.node = v;
                    }
                    if let Some((x, y)) = ss.pending.position.take() {
                        ss.x = x;
                        ss.y = y;
                    }
                }
                RoleData::XdgSurface(xdg) => {
                    if let Some(extents) = xdg.pending.extents.take() {
                        xdg.extents = Some(extents);
                    }
                    xdg_extents = xdg.extents;
                    if let XdgSurfaceRoleData::Toplevel(tl) = &xdg.role_data {
                        td_node = tl.node.as_ref().map(|n| n.node.clone());
                    }
                }
            }
        }
        {
            let mut pfr = self.pending.frame_request.borrow_mut();
            self.frame_requests.borrow_mut().extend(pfr.drain(..));
        }
        {
            if let Some(region) = self.pending.input_region.take() {
                self.input_region.set(Some(region));
            }
            if let Some(region) = self.pending.opaque_region.take() {
                self.opaque_region.set(Some(region));
            }
        }
        let mut committed_any_children = false;
        {
            let children = self.children.borrow();
            if let Some(children) = children.deref() {
                for child in children.subsurfaces.values() {
                    child.do_commit();
                    committed_any_children = true;
                }
            }
        }
        if !committed_any_children {
            self.calculate_extents();
        }
        let mut effective_extents = self.extents.get();
        if let Some(extents) = xdg_extents {
            effective_extents.x1 = effective_extents.x1.max(extents.x1);
            effective_extents.y1 = effective_extents.y1.max(extents.y1);
            effective_extents.x2 = effective_extents.x2.min(extents.x2);
            effective_extents.y2 = effective_extents.y2.min(extents.y2);
            if effective_extents.x1 > effective_extents.x2 {
                effective_extents.x1 = 0;
                effective_extents.x2 = 0;
            }
            if effective_extents.y1 > effective_extents.y2 {
                effective_extents.y1 = 0;
                effective_extents.y2 = 0;
            }
        }
        if let Some(node) = td_node {
            let mut td_extents = node.common.extents.get();
            td_extents.width = (effective_extents.x2 - effective_extents.x1) as u32;
            td_extents.height = (effective_extents.y2 - effective_extents.y1) as u32;
            node.common.extents.set(td_extents);
        }
        self.effective_extents.set(effective_extents);
    }

    async fn commit(&self, parser: MsgParser<'_, '_>) -> Result<(), CommitError> {
        let _req: Commit = self.parse(parser)?;
        {
            let rd = self.role_data.borrow();
            match rd.deref() {
                RoleData::Subsurface(ss) => {
                    if ss.sync_ancestor || ss.sync_requested {
                        return Ok(());
                    }
                }
                RoleData::XdgSurface(xdg) => {
                    if xdg.acked_serial != Some(xdg.requested_serial) {
                        if xdg.acked_serial.is_none() {
                            self.client
                                .event(xdg.xdg_surface.configure(xdg.requested_serial))
                                .await?;
                        }
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        self.do_commit();
        Ok(())
    }

    async fn set_buffer_transform(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), SetBufferTransformError> {
        let _req: SetBufferTransform = self.parse(parser)?;
        Ok(())
    }

    async fn set_buffer_scale(&self, parser: MsgParser<'_, '_>) -> Result<(), SetBufferScaleError> {
        let _req: SetBufferScale = self.parse(parser)?;
        Ok(())
    }

    async fn damage_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), DamageBufferError> {
        let _req: DamageBuffer = self.parse(parser)?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSurfaceError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            ATTACH => self.attach(parser).await?,
            DAMAGE => self.damage(parser).await?,
            FRAME => self.frame(parser).await?,
            SET_OPAQUE_REGION => self.set_opaque_region(parser).await?,
            SET_INPUT_REGION => self.set_input_region(parser).await?,
            COMMIT => self.commit(parser).await?,
            SET_BUFFER_TRANSFORM => self.set_buffer_transform(parser).await?,
            SET_BUFFER_SCALE => self.set_buffer_scale(parser).await?,
            DAMAGE_BUFFER => self.damage_buffer(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlSurface);

impl Object for WlSurface {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlSurface
    }

    fn num_requests(&self) -> u32 {
        DAMAGE_BUFFER + 1
    }

    fn break_loops(&self) {
        *self.children.borrow_mut() = None;
        *self.role_data.borrow_mut() = RoleData::None;
        mem::take(self.frame_requests.borrow_mut().deref_mut());
        self.buffer.set(None);
    }
}
