use {
    crate::{
        client::{Client, ClientError},
        ifs::ipc::{
            add_data_source_mime_type, break_source_loops, destroy_data_source,
            zwp_primary_selection_device_v1::PrimarySelectionIpc, SourceData,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_primary_selection_source_v1::*, ZwpPrimarySelectionSourceV1Id},
        xwayland::XWaylandEvent,
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct ZwpPrimarySelectionSourceV1 {
    pub id: ZwpPrimarySelectionSourceV1Id,
    pub data: SourceData<PrimarySelectionIpc>,
    pub tracker: Tracker<Self>,
}

impl ZwpPrimarySelectionSourceV1 {
    pub fn new(id: ZwpPrimarySelectionSourceV1Id, client: &Rc<Client>, is_xwm: bool) -> Self {
        Self {
            id,
            data: SourceData::new(client, is_xwm),
            tracker: Default::default(),
        }
    }

    pub fn send_cancelled(self: &Rc<Self>) {
        if self.data.is_xwm {
            self.data
                .client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::PrimarySelectionCancelSource(self.clone()));
        } else {
            self.data.client.event(Cancelled { self_id: self.id });
        }
    }

    pub fn send_send(self: &Rc<Self>, mime_type: &str, fd: Rc<OwnedFd>) {
        if self.data.is_xwm {
            self.data
                .client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::PrimarySelectionSendSource(
                    self.clone(),
                    mime_type.to_string(),
                    fd,
                ));
        } else {
            self.data.client.event(Send {
                self_id: self.id,
                mime_type,
                fd,
            })
        }
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
