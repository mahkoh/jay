use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::{
                add_data_source_mime_type, break_source_loops, cancel_offers, destroy_data_source,
                detach_seat, offer_source_to_regular_client, offer_source_to_wlr_device,
                offer_source_to_x,
                wl_data_device::ClipboardIpc,
                x_data_device::{XClipboardIpc, XIpcDevice, XPrimarySelectionIpc},
                zwlr_data_control_device_v1::{
                    WlrClipboardIpc, WlrPrimarySelectionIpc, ZwlrDataControlDeviceV1,
                },
                zwp_primary_selection_device_v1::PrimarySelectionIpc,
                DataSource, DynDataSource, IpcLocation, SourceData,
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwlr_data_control_source_v1::*, ZwlrDataControlSourceV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct ZwlrDataControlSourceV1 {
    pub id: ZwlrDataControlSourceV1Id,
    pub data: SourceData,
    pub version: u32,
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

    fn offer_to_regular_client(self: Rc<Self>, client: &Rc<Client>) {
        match self.location.get() {
            IpcLocation::Clipboard => {
                offer_source_to_regular_client::<ClipboardIpc, Self>(&self, client)
            }
            IpcLocation::PrimarySelection => {
                offer_source_to_regular_client::<PrimarySelectionIpc, Self>(&self, client)
            }
        }
    }

    fn offer_to_x(self: Rc<Self>, dd: &Rc<XIpcDevice>) {
        match self.location.get() {
            IpcLocation::Clipboard => offer_source_to_x::<XClipboardIpc, Self>(&self, dd),
            IpcLocation::PrimarySelection => {
                offer_source_to_x::<XPrimarySelectionIpc, Self>(&self, dd)
            }
        }
    }

    fn offer_to_wlr_device(self: Rc<Self>, dd: &Rc<ZwlrDataControlDeviceV1>) {
        match self.location.get() {
            IpcLocation::Clipboard => {
                offer_source_to_wlr_device::<WlrClipboardIpc, Self>(&self, dd)
            }
            IpcLocation::PrimarySelection => {
                offer_source_to_wlr_device::<WlrPrimarySelectionIpc, Self>(&self, dd)
            }
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
    pub fn new(id: ZwlrDataControlSourceV1Id, client: &Rc<Client>, version: u32) -> Self {
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

    fn offer(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrDataControlSourceV1Error> {
        let req: Offer = self.data.client.parse(self, parser)?;
        if self.used.get() {
            return Err(ZwlrDataControlSourceV1Error::AlreadyUsed);
        }
        add_data_source_mime_type::<WlrClipboardIpc>(self, req.mime_type);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrDataControlSourceV1Error> {
        let _req: Destroy = self.data.client.parse(self, parser)?;
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

    OFFER => offer,
    DESTROY => destroy,
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
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The source has already been used")]
    AlreadyUsed,
}
efrom!(ZwlrDataControlSourceV1Error, ClientError);
efrom!(ZwlrDataControlSourceV1Error, MsgParserError);
