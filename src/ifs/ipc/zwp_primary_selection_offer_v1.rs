use {
    crate::{
        client::{Client, ClientError},
        ifs::ipc::{
            break_offer_loops, destroy_offer, receive,
            zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1, OfferData,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_primary_selection_offer_v1::*, ZwpPrimarySelectionOfferV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpPrimarySelectionOfferV1 {
    pub id: ZwpPrimarySelectionOfferV1Id,
    pub client: Rc<Client>,
    pub offer_data: OfferData<ZwpPrimarySelectionDeviceV1>,
    pub tracker: Tracker<Self>,
}

impl ZwpPrimarySelectionOfferV1 {
    pub fn send_offer(&self, mime_type: &str) {
        self.client.event(Offer {
            self_id: self.id,
            mime_type,
        })
    }

    fn receive(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionOfferV1Error> {
        let req: Receive = self.client.parse(self, parser)?;
        receive::<ZwpPrimarySelectionDeviceV1>(self, req.mime_type, req.fd);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionOfferV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        destroy_offer::<ZwpPrimarySelectionDeviceV1>(self);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    ZwpPrimarySelectionOfferV1;

    RECEIVE => receive,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionOfferV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        break_offer_loops::<ZwpPrimarySelectionDeviceV1>(self);
    }
}

simple_add_obj!(ZwpPrimarySelectionOfferV1);

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionOfferV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPrimarySelectionOfferV1Error, ClientError);
efrom!(ZwpPrimarySelectionOfferV1Error, MsgParserError);
