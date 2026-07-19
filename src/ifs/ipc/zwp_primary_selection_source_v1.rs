use crate::client::Client;
use crate::client::ClientError;
use crate::ifs::ipc::DataSource;
use crate::ifs::ipc::DynDataSource;
use crate::ifs::ipc::SourceData;
use crate::ifs::ipc::add_data_source_mime_type;
use crate::ifs::ipc::break_source_loops;
use crate::ifs::ipc::cancel_offers;
use crate::ifs::ipc::destroy_data_source;
use crate::ifs::ipc::detach_seat;
use crate::ifs::ipc::offer_source_to_x;
use crate::ifs::ipc::x_data_device::XIpcDevice;
use crate::ifs::ipc::x_data_device::XPrimarySelectionIpc;
use crate::ifs::ipc::zwp_primary_selection_device_v1::PrimarySelectionIpc;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::wire::ZwpPrimarySelectionSourceV1Id;
use crate::wire::zwp_primary_selection_source_v1::*;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

pub struct ZwpPrimarySelectionSourceV1 {
    pub id: ZwpPrimarySelectionSourceV1Id,
    pub data: SourceData,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl DataSource for ZwpPrimarySelectionSourceV1 {
    fn send_cancelled(&self, _seat: &Rc<WlSeatGlobal>) {
        ZwpPrimarySelectionSourceV1::send_cancelled(self);
    }
}

impl DynDataSource for ZwpPrimarySelectionSourceV1 {
    fn source_data(&self) -> &SourceData {
        &self.data
    }

    fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        ZwpPrimarySelectionSourceV1::send_send(self, mime_type, fd)
    }

    fn offer_to_x(self: Rc<Self>, dd: &Rc<XIpcDevice>) {
        offer_source_to_x::<XPrimarySelectionIpc>(self, dd);
    }

    fn detach_seat(&self, seat: &Rc<WlSeatGlobal>) {
        detach_seat(self, seat);
    }

    fn cancel_unprivileged_offers(&self) {
        cancel_offers(self, false);
    }
}

impl ZwpPrimarySelectionSourceV1 {
    pub fn new(id: ZwpPrimarySelectionSourceV1Id, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            data: SourceData::new(client),
            tracker: Default::default(),
            version,
        }
    }

    pub fn send_cancelled(&self) {
        self.data.client.event(Cancelled { self_id: self.id });
    }

    pub fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.data.client.event(Send {
            self_id: self.id,
            mime_type,
            fd,
        })
    }
}

impl ZwpPrimarySelectionSourceV1RequestHandler for ZwpPrimarySelectionSourceV1 {
    type Error = ZwpPrimarySelectionSourceV1Error;

    fn offer(&self, req: Offer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        add_data_source_mime_type::<PrimarySelectionIpc>(self, req.mime_type);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        destroy_data_source::<PrimarySelectionIpc>(self);
        self.data.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpPrimarySelectionSourceV1;
    version = self.version;
}

impl Object for ZwpPrimarySelectionSourceV1 {
    fn break_loops(self: Rc<Self>) {
        break_source_loops::<PrimarySelectionIpc>(&*self);
    }
}

dedicated_add_obj!(
    ZwpPrimarySelectionSourceV1,
    ZwpPrimarySelectionSourceV1Id,
    zwp_primary_selection_source
);

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionSourceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPrimarySelectionSourceV1Error, ClientError);
