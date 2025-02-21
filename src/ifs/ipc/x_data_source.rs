use {
    crate::{
        ifs::{
            ipc::{
                DataSource, DynDataSource, IpcLocation, SourceData, cancel_offers, detach_seat,
                x_data_device::XIpcDevice,
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

    fn offer_to_x(self: Rc<Self>, _dd: &Rc<XIpcDevice>) {
        self.cancel_unprivileged_offers();
        self.state.xwayland.queue.push(IpcSetSelection {
            location: self.location,
            seat: self.device.seat.id(),
            offer: None,
        });
    }

    fn detach_seat(&self, seat: &Rc<WlSeatGlobal>) {
        detach_seat(self, seat);
    }

    fn cancel_unprivileged_offers(&self) {
        cancel_offers(self, false)
    }
}
