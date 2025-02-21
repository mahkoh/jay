use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::{
                DataSource, DynDataSource, SourceData, add_data_source_mime_type,
                break_source_loops, cancel_offers, destroy_data_source, detach_seat,
                offer_source_to_x,
                x_data_device::{XIpcDevice, XPrimarySelectionIpc},
                zwp_primary_selection_device_v1::PrimarySelectionIpc,
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpPrimarySelectionSourceV1Id, zwp_primary_selection_source_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::OwnedFd,
};

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
    fn break_loops(&self) {
        break_source_loops::<PrimarySelectionIpc>(self);
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
