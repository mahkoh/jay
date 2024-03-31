use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::{
                add_data_source_mime_type, break_source_loops, cancel_offers, destroy_data_source,
                detach_seat, offer_source_to_regular_client, offer_source_to_x,
                x_data_device::{XIpcDevice, XPrimarySelectionIpc},
                zwp_primary_selection_device_v1::PrimarySelectionIpc,
                DataSource, DynDataSource, SourceData,
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_primary_selection_source_v1::*, ZwpPrimarySelectionSourceV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct ZwpPrimarySelectionSourceV1 {
    pub id: ZwpPrimarySelectionSourceV1Id,
    pub data: SourceData,
    pub tracker: Tracker<Self>,
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

    fn offer_to_regular_client(self: Rc<Self>, client: &Rc<Client>) {
        offer_source_to_regular_client::<PrimarySelectionIpc, Self>(&self, client);
    }

    fn offer_to_x(self: Rc<Self>, dd: &Rc<XIpcDevice>) {
        offer_source_to_x::<XPrimarySelectionIpc, Self>(&self, dd);
    }

    fn detach_seat(&self, seat: &Rc<WlSeatGlobal>) {
        detach_seat(self, seat);
    }

    fn cancel_offers(&self) {
        cancel_offers(self);
    }
}

impl ZwpPrimarySelectionSourceV1 {
    pub fn new(id: ZwpPrimarySelectionSourceV1Id, client: &Rc<Client>) -> Self {
        Self {
            id,
            data: SourceData::new(client),
            tracker: Default::default(),
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

    fn offer(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionSourceV1Error> {
        let req: Offer = self.data.client.parse(self, parser)?;
        add_data_source_mime_type::<PrimarySelectionIpc>(self, req.mime_type);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionSourceV1Error> {
        let _req: Destroy = self.data.client.parse(self, parser)?;
        destroy_data_source::<PrimarySelectionIpc>(self);
        self.data.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpPrimarySelectionSourceV1;

    OFFER => offer,
    DESTROY => destroy,
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
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(ZwpPrimarySelectionSourceV1Error, ClientError);
efrom!(ZwpPrimarySelectionSourceV1Error, MsgParserError);
