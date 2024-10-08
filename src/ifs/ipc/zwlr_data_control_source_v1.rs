use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::{
                add_data_source_mime_type, break_source_loops, cancel_offers, destroy_data_source,
                detach_seat, offer_source_to_x,
                x_data_device::{XClipboardIpc, XIpcDevice, XPrimarySelectionIpc},
                zwlr_data_control_device_v1::{WlrClipboardIpc, WlrPrimarySelectionIpc},
                DataSource, DynDataSource, IpcLocation, SourceData,
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwlr_data_control_source_v1::*, ZwlrDataControlSourceV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct ZwlrDataControlSourceV1 {
    pub id: ZwlrDataControlSourceV1Id,
    pub data: SourceData,
    pub version: Version,
    pub location: Cell<IpcLocation>,
    pub used: Cell<bool>,
    pub tracker: Tracker<Self>,
}

impl DataSource for ZwlrDataControlSourceV1 {
    fn send_cancelled(&self, _seat: &Rc<WlSeatGlobal>) {
        ZwlrDataControlSourceV1::send_cancelled(self);
    }
}

impl DynDataSource for ZwlrDataControlSourceV1 {
    fn source_data(&self) -> &SourceData {
        &self.data
    }

    fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        ZwlrDataControlSourceV1::send_send(&self, mime_type, fd);
    }

    fn offer_to_x(self: Rc<Self>, dd: &Rc<XIpcDevice>) {
        match self.location.get() {
            IpcLocation::Clipboard => offer_source_to_x::<XClipboardIpc>(self, dd),
            IpcLocation::PrimarySelection => offer_source_to_x::<XPrimarySelectionIpc>(self, dd),
        }
    }

    fn detach_seat(&self, seat: &Rc<WlSeatGlobal>) {
        detach_seat(self, seat)
    }

    fn cancel_unprivileged_offers(&self) {
        cancel_offers(self, false)
    }
}

impl ZwlrDataControlSourceV1 {
    pub fn new(id: ZwlrDataControlSourceV1Id, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            tracker: Default::default(),
            data: SourceData::new(client),
            version,
            location: Cell::new(IpcLocation::Clipboard),
            used: Cell::new(false),
        }
    }

    pub fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.data.client.event(Send {
            self_id: self.id,
            mime_type,
            fd,
        })
    }

    pub fn send_cancelled(&self) {
        self.data.client.event(Cancelled { self_id: self.id })
    }
}

impl ZwlrDataControlSourceV1RequestHandler for ZwlrDataControlSourceV1 {
    type Error = ZwlrDataControlSourceV1Error;

    fn offer(&self, req: Offer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.used.get() {
            return Err(ZwlrDataControlSourceV1Error::AlreadyUsed);
        }
        add_data_source_mime_type::<WlrClipboardIpc>(self, req.mime_type);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        match self.location.get() {
            IpcLocation::Clipboard => destroy_data_source::<WlrClipboardIpc>(self),
            IpcLocation::PrimarySelection => destroy_data_source::<WlrPrimarySelectionIpc>(self),
        }
        self.data.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrDataControlSourceV1;
    version = self.version;
}

impl Object for ZwlrDataControlSourceV1 {
    fn break_loops(&self) {
        match self.location.get() {
            IpcLocation::Clipboard => break_source_loops::<WlrClipboardIpc>(self),
            IpcLocation::PrimarySelection => break_source_loops::<WlrPrimarySelectionIpc>(self),
        }
    }
}

dedicated_add_obj!(
    ZwlrDataControlSourceV1,
    ZwlrDataControlSourceV1Id,
    zwlr_data_sources
);

#[derive(Debug, Error)]
pub enum ZwlrDataControlSourceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The source has already been used")]
    AlreadyUsed,
}
efrom!(ZwlrDataControlSourceV1Error, ClientError);
