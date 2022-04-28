use {
    crate::{
        client::{Client, ClientError},
        ifs::ipc::{
            add_mime_type, break_source_loops, destroy_source,
            zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1, SourceData,
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
    pub data: SourceData<ZwpPrimarySelectionDeviceV1>,
    pub tracker: Tracker<Self>,
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
        self.data.client.event(Cancelled { self_id: self.id })
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
        add_mime_type::<ZwpPrimarySelectionDeviceV1>(self, req.mime_type);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionSourceV1Error> {
        let _req: Destroy = self.data.client.parse(self, parser)?;
        destroy_source::<ZwpPrimarySelectionDeviceV1>(self);
        self.data.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    ZwpPrimarySelectionSourceV1;

    OFFER => offer,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionSourceV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        break_source_loops::<ZwpPrimarySelectionDeviceV1>(self);
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
