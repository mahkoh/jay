use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::{
                x_data_offer::XDataOffer, x_data_source::XDataSource, DeviceData, IpcLocation,
                IpcVtable, OfferData, Role,
            },
            wl_seat::{WlSeatError, WlSeatGlobal},
        },
        state::State,
        xwayland::XWaylandEvent,
    },
    std::rc::Rc,
    XWaylandEvent::IpcSetOffer,
};

linear_ids!(XIpcDeviceIds, XIpcDeviceId, u64);

pub struct XIpcDevice {
    pub id: XIpcDeviceId,
    pub clipboard: DeviceData<XDataOffer>,
    pub primary_selection: DeviceData<XDataOffer>,
    pub seat: Rc<WlSeatGlobal>,
    pub state: Rc<State>,
    pub client: Rc<Client>,
}

#[derive(Default)]
pub struct XClipboardIpc;

#[derive(Default)]
pub struct XPrimarySelectionIpc;

pub trait XIpc {
    const LOCATION: IpcLocation;

    fn x_unset(seat: &Rc<WlSeatGlobal>);

    fn x_device_data(dd: &XIpcDevice) -> &DeviceData<XDataOffer>;
}

impl XIpc for XClipboardIpc {
    const LOCATION: IpcLocation = IpcLocation::Clipboard;

    fn x_unset(seat: &Rc<WlSeatGlobal>) {
        seat.unset_selection();
    }

    fn x_device_data(dd: &XIpcDevice) -> &DeviceData<XDataOffer> {
        &dd.clipboard
    }
}

impl XIpc for XPrimarySelectionIpc {
    const LOCATION: IpcLocation = IpcLocation::PrimarySelection;

    fn x_unset(seat: &Rc<WlSeatGlobal>) {
        seat.unset_primary_selection();
    }

    fn x_device_data(dd: &XIpcDevice) -> &DeviceData<XDataOffer> {
        &dd.primary_selection
    }
}

impl<T: XIpc> IpcVtable for T {
    type Device = XIpcDevice;
    type Source = XDataSource;
    type Offer = XDataOffer;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self::Offer> {
        T::x_device_data(dd)
    }

    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal> {
        dd.seat.clone()
    }

    fn set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<Self::Source>,
        _serial: Option<u32>,
    ) -> Result<(), WlSeatError> {
        match source.location {
            IpcLocation::Clipboard => seat.set_selection(Some(source.clone())),
            IpcLocation::PrimarySelection => seat.set_primary_selection(Some(source.clone())),
        }
    }

    fn create_offer(
        dd: &Rc<Self::Device>,
        data: OfferData<Self::Device>,
    ) -> Result<Rc<Self::Offer>, ClientError> {
        debug_assert!(dd.client.is_xwayland);
        let rc = Rc::new(XDataOffer {
            offer_id: dd.state.data_offer_ids.next(),
            device: dd.clone(),
            data,
            tracker: Default::default(),
            location: T::LOCATION,
        });
        track!(dd.client, rc);
        Ok(rc)
    }

    fn send_selection(dd: &Self::Device, offer: Option<&Rc<Self::Offer>>) {
        dd.state
            .xwayland
            .queue
            .push(XWaylandEvent::IpcSetSelection {
                seat: dd.seat.id(),
                location: T::LOCATION,
                offer: offer.cloned(),
            });
    }

    fn send_offer(dd: &Self::Device, offer: &Rc<Self::Offer>) {
        dd.state.xwayland.queue.push(IpcSetOffer {
            location: T::LOCATION,
            seat: dd.seat.id(),
            offer: offer.clone(),
        });
    }

    fn unset(seat: &Rc<WlSeatGlobal>, _role: Role) {
        T::x_unset(seat)
    }

    fn device_client(dd: &Rc<Self::Device>) -> &Rc<Client> {
        &dd.client
    }
}
