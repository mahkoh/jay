use {
    crate::{
        client::{Client, ClientError, ClientId},
        ifs::{
            ipc::{
                break_device_loops, destroy_data_device,
                zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1,
                zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1, DeviceData,
                IpcVtable, OfferData, Role, SourceData,
            },
            wl_seat::{WlSeatError, WlSeatGlobal},
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{
            zwp_primary_selection_device_v1::*, ZwpPrimarySelectionDeviceV1Id,
            ZwpPrimarySelectionOfferV1Id, ZwpPrimarySelectionSourceV1Id,
        },
        xwayland::XWaylandEvent,
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct ZwpPrimarySelectionDeviceV1 {
    pub id: ZwpPrimarySelectionDeviceV1Id,
    pub client: Rc<Client>,
    pub version: u32,
    pub seat: Rc<WlSeatGlobal>,
    data: DeviceData<PrimarySelectionIpc>,
    pub tracker: Tracker<Self>,
}

impl ZwpPrimarySelectionDeviceV1 {
    pub fn new(
        id: ZwpPrimarySelectionDeviceV1Id,
        client: &Rc<Client>,
        version: u32,
        seat: &Rc<WlSeatGlobal>,
        is_xwm: bool,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            version,
            seat: seat.clone(),
            data: DeviceData {
                selection: Default::default(),
                dnd: Default::default(),
                is_xwm,
            },
            tracker: Default::default(),
        }
    }

    pub fn send_data_offer(&self, offer: &Rc<ZwpPrimarySelectionOfferV1>) {
        if self.data.is_xwm {
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::PrimarySelectionSetOffer(offer.clone()));
        } else {
            self.client.event(DataOffer {
                self_id: self.id,
                offer: offer.id,
            })
        }
    }

    pub fn send_selection(&self, offer: Option<&Rc<ZwpPrimarySelectionOfferV1>>) {
        if self.data.is_xwm {
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::PrimarySelectionSetSelection(
                    self.seat.id(),
                    offer.cloned(),
                ));
        } else {
            let id = offer
                .map(|o| o.id)
                .unwrap_or(ZwpPrimarySelectionOfferV1Id::NONE);
            self.client.event(Selection {
                self_id: self.id,
                id,
            })
        }
    }

    fn set_selection(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwpPrimarySelectionDeviceV1Error> {
        let req: SetSelection = self.client.parse(self, parser)?;
        if !self.client.valid_serial(req.serial) {
            log::warn!("Client tried to set_selection with an invalid serial");
            return Ok(());
        }
        if !self
            .seat
            .may_modify_primary_selection(&self.client, Some(req.serial))
        {
            log::warn!("Ignoring disallowed set_selection request");
            return Ok(());
        }
        let src = if req.source.is_none() {
            None
        } else {
            Some(self.client.lookup(req.source)?)
        };
        self.seat.set_primary_selection(src, Some(req.serial))?;
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionDeviceV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        destroy_data_device::<PrimarySelectionIpc>(self);
        self.seat.remove_primary_selection_device(self);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

pub struct PrimarySelectionIpc;

impl IpcVtable for PrimarySelectionIpc {
    type Device = ZwpPrimarySelectionDeviceV1;
    type Source = ZwpPrimarySelectionSourceV1;
    type Offer = ZwpPrimarySelectionOfferV1;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self> {
        &dd.data
    }

    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal> {
        dd.seat.clone()
    }

    fn create_xwm_source(client: &Rc<Client>) -> Self::Source {
        ZwpPrimarySelectionSourceV1::new(ZwpPrimarySelectionSourceV1Id::NONE, client, true)
    }

    fn set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<Self::Source>,
        serial: Option<u32>,
    ) -> Result<(), WlSeatError> {
        seat.set_primary_selection(Some(source.clone()), serial)
    }

    fn get_offer_data(offer: &Self::Offer) -> &OfferData<Self> {
        &offer.data
    }

    fn get_source_data(src: &Self::Source) -> &SourceData<Self> {
        &src.data
    }

    fn for_each_device<C>(seat: &WlSeatGlobal, client: ClientId, f: C)
    where
        C: FnMut(&Rc<Self::Device>),
    {
        seat.for_each_primary_selection_device(0, client, f)
    }

    fn create_offer(
        client: &Rc<Client>,
        device: &Rc<ZwpPrimarySelectionDeviceV1>,
        offer_data: OfferData<Self>,
    ) -> Result<Rc<Self::Offer>, ClientError> {
        let id = if device.data.is_xwm {
            ZwpPrimarySelectionOfferV1Id::NONE
        } else {
            client.new_id()?
        };
        let rc = Rc::new(ZwpPrimarySelectionOfferV1 {
            id,
            u64_id: client.state.data_offer_ids.fetch_add(1),
            seat: device.seat.clone(),
            client: client.clone(),
            data: offer_data,
            tracker: Default::default(),
        });
        track!(client, rc);
        Ok(rc)
    }

    fn send_selection(dd: &Self::Device, offer: Option<&Rc<Self::Offer>>) {
        dd.send_selection(offer);
    }

    fn send_cancelled(source: &Rc<Self::Source>) {
        source.send_cancelled();
    }

    fn get_offer_id(offer: &Self::Offer) -> u64 {
        offer.u64_id
    }

    fn send_offer(dd: &Self::Device, offer: &Rc<Self::Offer>) {
        dd.send_data_offer(offer);
    }

    fn send_mime_type(offer: &Rc<Self::Offer>, mime_type: &str) {
        offer.send_offer(mime_type);
    }

    fn unset(seat: &Rc<WlSeatGlobal>, _role: Role) {
        seat.unset_primary_selection();
    }

    fn send_send(src: &Rc<Self::Source>, mime_type: &str, fd: Rc<OwnedFd>) {
        src.send_send(mime_type, fd);
    }

    fn remove_from_seat(device: &Self::Device) {
        device.seat.remove_primary_selection_device(device);
    }

    fn get_offer_seat(offer: &Self::Offer) -> Rc<WlSeatGlobal> {
        offer.seat.clone()
    }

    fn source_eq(left: &Self::Source, right: &Self::Source) -> bool {
        left as *const _ == right as *const _
    }
}

object_base! {
    self = ZwpPrimarySelectionDeviceV1;

    SET_SELECTION => set_selection,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionDeviceV1 {
    fn break_loops(&self) {
        break_device_loops::<PrimarySelectionIpc>(self);
        self.seat.remove_primary_selection_device(self);
    }
}

simple_add_obj!(ZwpPrimarySelectionDeviceV1);

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionDeviceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    WlSeatError(Box<WlSeatError>),
}
efrom!(ZwpPrimarySelectionDeviceV1Error, ClientError);
efrom!(ZwpPrimarySelectionDeviceV1Error, MsgParserError);
efrom!(ZwpPrimarySelectionDeviceV1Error, WlSeatError);
