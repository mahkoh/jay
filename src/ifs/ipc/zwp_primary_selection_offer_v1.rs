use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::{
                break_offer_loops, destroy_data_offer, receive_data_offer,
                zwp_primary_selection_device_v1::PrimarySelectionIpc, OfferData,
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_primary_selection_offer_v1::*, ZwpPrimarySelectionOfferV1Id},
        xwayland::XWaylandEvent,
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpPrimarySelectionOfferV1 {
    pub id: ZwpPrimarySelectionOfferV1Id,
    pub u64_id: u64,
    pub seat: Rc<WlSeatGlobal>,
    pub client: Rc<Client>,
    pub data: OfferData<PrimarySelectionIpc>,
    pub tracker: Tracker<Self>,
}

impl ZwpPrimarySelectionOfferV1 {
    pub fn send_offer(self: &Rc<Self>, mime_type: &str) {
        if self.data.is_xwm {
            if let Some(src) = self.data.source.get() {
                if !src.data.is_xwm {
                    self.client.state.xwayland.queue.push(
                        XWaylandEvent::PrimarySelectionAddOfferMimeType(
                            self.clone(),
                            mime_type.to_string(),
                        ),
                    );
                }
            }
        } else {
            self.client.event(Offer {
                self_id: self.id,
                mime_type,
            })
        }
    }

    fn receive(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionOfferV1Error> {
        let req: Receive = self.client.parse(self, parser)?;
        receive_data_offer::<PrimarySelectionIpc>(self, req.mime_type, req.fd);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionOfferV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        destroy_data_offer::<PrimarySelectionIpc>(self);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpPrimarySelectionOfferV1;

    RECEIVE => receive,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionOfferV1 {
    fn break_loops(&self) {
        break_offer_loops::<PrimarySelectionIpc>(self);
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
