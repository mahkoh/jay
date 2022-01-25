mod types;

use crate::client::Client;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const START_DRAG: u32 = 0;
const SET_SELECTION: u32 = 1;
const RELEASE: u32 = 2;

const DATA_OFFER: u32 = 0;
const ENTER: u32 = 1;
const LEAVE: u32 = 2;
const MOTION: u32 = 4;
const DROP: u32 = 5;
const SELECTION: u32 = 5;

#[allow(dead_code)]
const ROLE: u32 = 0;

id!(WlDataDeviceId);

pub struct WlDataDevice {
    id: WlDataDeviceId,
    client: Rc<Client>,
}

impl WlDataDevice {
    pub fn new(id: WlDataDeviceId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
        }
    }

    fn start_drag(&self, parser: MsgParser<'_, '_>) -> Result<(), StartDragError> {
        let _req: StartDrag = self.client.parse(self, parser)?;
        Ok(())
    }

    fn set_selection(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSelectionError> {
        let _req: SetSelection = self.client.parse(self, parser)?;
        Ok(())
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDataDeviceError> {
        match request {
            START_DRAG => self.start_drag(parser)?,
            SET_SELECTION => self.set_selection(parser)?,
            RELEASE => self.release(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlDataDevice);

impl Object for WlDataDevice {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlDataDevice
    }

    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }
}
