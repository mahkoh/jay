mod types;

use crate::ifs::wl_surface::{SubsurfaceData, SurfaceType, WlSurface};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const SET_POSITION: u32 = 1;
const PLACE_ABOVE: u32 = 2;
const PLACE_BELOW: u32 = 3;
const SET_SYNC: u32 = 4;
const SET_DESYNC: u32 = 5;

const BAD_SURFACE: u32 = 0;

pub struct WlSubsurface {
    id: ObjectId,
    surface: Rc<WlSurface>,
}

impl WlSubsurface {
    pub fn new(id: ObjectId, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
            surface: surface.clone(),
        }
    }

    pub fn install(self: &Rc<Self>, parent: &Rc<WlSurface>) -> Result<(), WlSubsurfaceError> {
        let old_ty = self.surface.ty.get();
        if !matches!(old_ty, SurfaceType::None | SurfaceType::Subsurface) {
            return Err(WlSubsurfaceError::IncompatibleType(self.surface.id, old_ty));
        }
        self.surface.ty.set(SurfaceType::Subsurface);
        let mut data = self.surface.subsurface_data.borrow_mut();
        if data.is_some() {
            return Err(WlSubsurfaceError::AlreadyAttached(self.surface.id));
        }
        if self.surface.id == parent.id {
            return Err(WlSubsurfaceError::OwnParent(self.surface.id));
        }
        if self.surface.id == parent.get_root().id {
            return Err(WlSubsurfaceError::Ancestor(self.surface.id, parent.id));
        }
        let mut sync_ancestor = false;
        {
            let data = parent.subsurface_data.borrow();
            if let Some(data) = data.as_ref() {
                sync_ancestor = data.sync_requested || data.sync_ancestor;
            }
        }
        *data = Some(Box::new(SubsurfaceData {
            subsurface: self.clone(),
            parent: parent.clone(),
            sync_requested: false,
            sync_ancestor,
            pending: true,
        }));
        {
            let mut data = parent.children.borrow_mut();
            let data = data.get_or_insert_with(|| Default::default());
            data.pending_subsurfaces
                .insert(self.surface.id, self.surface.clone());
        }
        Ok(())
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_position(&self, parser: MsgParser<'_, '_>) -> Result<(), SetPositionError> {
        let req: SetPosition = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn place_above(&self, parser: MsgParser<'_, '_>) -> Result<(), PlaceAboveError> {
        let req: PlaceAbove = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn place_below(&self, parser: MsgParser<'_, '_>) -> Result<(), PlaceBelowError> {
        let req: PlaceBelow = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_sync(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSyncError> {
        let _req: SetSync = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_desync(&self, parser: MsgParser<'_, '_>) -> Result<(), SetDesyncError> {
        let _req: SetDesync = self.surface.client.parse(self, parser)?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSubsurfaceError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            SET_POSITION => self.set_position(parser).await?,
            PLACE_ABOVE => self.place_above(parser).await?,
            PLACE_BELOW => self.place_below(parser).await?,
            SET_SYNC => self.set_sync(parser).await?,
            SET_DESYNC => self.set_desync(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlSubsurface);

impl Object for WlSubsurface {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::WlSubsurface
    }

    fn num_requests(&self) -> u32 {
        SET_DESYNC + 1
    }
}
