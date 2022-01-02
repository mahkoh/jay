mod types;
pub mod wl_subsurface;

use crate::client::{Client, RequestParser};
use crate::ifs::wl_surface::wl_subsurface::WlSubsurface;
use crate::object::{Interface, Object, ObjectId};
use crate::pixman::Region;
use crate::utils::buffd::{MsgParser, MsgParserError};
use ahash::AHashMap;
use std::cell::{Cell, RefCell};
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

const ENTER: u32 = 0;
const LEAVE: u32 = 1;

const INVALID_SCALE: u32 = 0;
const INVALID_TRANSFORM: u32 = 1;
const INVALID_SIZE: u32 = 2;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SurfaceType {
    None,
    Subsurface,
}

pub struct WlSurface {
    id: ObjectId,
    client: Rc<Client>,
    ty: Cell<SurfaceType>,
    pending: PendingState,
    children: RefCell<Option<Box<ParentData>>>,
    subsurface_data: RefCell<Option<Box<SubsurfaceData>>>,
}

#[derive(Default)]
struct PendingState {
    opaque_region: Cell<Option<Region>>,
    input_region: Cell<Option<Region>>,
}

struct SubsurfaceData {
    subsurface: Rc<WlSubsurface>,
    parent: Rc<WlSurface>,
    sync_requested: bool,
    sync_ancestor: bool,
    pending: bool,
}

#[derive(Default)]
struct ParentData {
    subsurfaces: AHashMap<ObjectId, Rc<WlSurface>>,
    pending_subsurfaces: AHashMap<ObjectId, Rc<WlSurface>>,
}

impl WlSurface {
    pub fn new(id: ObjectId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            ty: Cell::new(SurfaceType::None),
            pending: Default::default(),
            children: Default::default(),
            subsurface_data: Default::default(),
        }
    }

    pub fn break_loops(&self) {
        *self.children.borrow_mut() = None;
        *self.subsurface_data.borrow_mut() = None;
    }

    pub fn get_root(self: &Rc<Self>) -> Rc<WlSurface> {
        let mut root = self.clone();
        loop {
            let tmp = root;
            let data = tmp.subsurface_data.borrow();
            match data.as_ref() {
                Some(d) => root = d.parent.clone(),
                None => {
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
        let destroy: Destroy = self.parse(parser)?;
        Ok(())
    }

    async fn attach(&self, parser: MsgParser<'_, '_>) -> Result<(), AttachError> {
        let attach: Attach = self.parse(parser)?;
        Ok(())
    }

    async fn damage(&self, parser: MsgParser<'_, '_>) -> Result<(), DamageError> {
        let damage: Damage = self.parse(parser)?;
        Ok(())
    }

    async fn frame(&self, parser: MsgParser<'_, '_>) -> Result<(), FrameError> {
        let frame: Frame = self.parse(parser)?;
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
        let region: SetInputRegion = self.parse(parser)?;
        let region = self.client.get_region(region.region)?;
        self.pending.input_region.set(Some(region.region()));
        Ok(())
    }

    async fn commit(&self, parser: MsgParser<'_, '_>) -> Result<(), CommitError> {
        let commit: Commit = self.parse(parser)?;
        Ok(())
    }

    async fn set_buffer_transform(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), SetBufferTransformError> {
        let transform: SetBufferTransform = self.parse(parser)?;
        Ok(())
    }

    async fn set_buffer_scale(&self, parser: MsgParser<'_, '_>) -> Result<(), SetBufferScaleError> {
        let scale: SetBufferScale = self.parse(parser)?;
        Ok(())
    }

    async fn damage_buffer(&self, parser: MsgParser<'_, '_>) -> Result<(), DamageBufferError> {
        let damage: DamageBuffer = self.parse(parser)?;
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
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::WlSurface
    }

    fn num_requests(&self) -> u32 {
        DAMAGE_BUFFER + 1
    }
}
