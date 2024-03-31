use {
    crate::{
        client::Client,
        ifs::{
            ipc::{
                cancel_offers, detach_seat, offer_source_to_regular_client,
                offer_source_to_wlr_device,
                wl_data_device::ClipboardIpc,
                x_data_device::XIpcDevice,
                zwlr_data_control_device_v1::{
                    WlrClipboardIpc, WlrPrimarySelectionIpc, ZwlrDataControlDeviceV1,
                },
                zwp_primary_selection_device_v1::PrimarySelectionIpc,
                DataSource, DynDataSource, IpcLocation, SourceData,
            },
            wl_seat::WlSeatGlobal,
        },
        state::State,
        xwayland::XWaylandEvent::{IpcCancelSource, IpcSendSource, IpcSetSelection},
    },
    std::rc::Rc,
    uapi::OwnedFd,
};

pub struct XDataSource {
    pub state: Rc<State>,
    pub device: Rc<XIpcDevice>,
    pub data: SourceData,
    pub location: IpcLocation,
}

impl DataSource for XDataSource {
    fn send_cancelled(&self, seat: &Rc<WlSeatGlobal>) {
        self.state.xwayland.queue.push(IpcCancelSource {
            location: self.location,
            seat: seat.id(),
            source: self.data.id,
        });
    }
}

impl DynDataSource for XDataSource {
    fn source_data(&self) -> &SourceData {
        &self.data
    }

    fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.state.xwayland.queue.push(IpcSendSource {
            location: self.location,
            seat: self.device.seat.id(),
            source: self.data.id,
            mime_type: mime_type.to_string(),
            fd,
        });
    }

    fn offer_to_regular_client(self: Rc<Self>, client: &Rc<Client>) {
        match self.location {
            IpcLocation::Clipboard => {
                offer_source_to_regular_client::<ClipboardIpc, Self>(&self, client)
            }
            IpcLocation::PrimarySelection => {
                offer_source_to_regular_client::<PrimarySelectionIpc, Self>(&self, client)
            }
        }
    }

    fn offer_to_x(self: Rc<Self>, _dd: &Rc<XIpcDevice>) {
        self.cancel_unprivileged_offers();
        self.state.xwayland.queue.push(IpcSetSelection {
            location: self.location,
            seat: self.device.seat.id(),
            offer: None,
        });
    }

    fn offer_to_wlr_device(self: Rc<Self>, dd: &Rc<ZwlrDataControlDeviceV1>) {
        match self.location {
            IpcLocation::Clipboard => {
                offer_source_to_wlr_device::<WlrClipboardIpc, Self>(&self, dd)
            }
            IpcLocation::PrimarySelection => {
                offer_source_to_wlr_device::<WlrPrimarySelectionIpc, Self>(&self, dd)
            }
        }
    }

    fn detach_seat(&self, seat: &Rc<WlSeatGlobal>) {
        detach_seat(self, seat);
    }

    fn cancel_unprivileged_offers(&self) {
        cancel_offers(self, false)
    }
}
