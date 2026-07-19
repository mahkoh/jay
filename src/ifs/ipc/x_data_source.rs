use crate::ifs::ipc::DataSource;
use crate::ifs::ipc::DynDataSource;
use crate::ifs::ipc::IpcLocation;
use crate::ifs::ipc::SourceData;
use crate::ifs::ipc::cancel_offers;
use crate::ifs::ipc::detach_seat;
use crate::ifs::ipc::x_data_device::XIpcDevice;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::state::State;
use crate::xwayland::XWaylandEvent::IpcCancelSource;
use crate::xwayland::XWaylandEvent::IpcSendSource;
use crate::xwayland::XWaylandEvent::IpcSetSelection;
use std::rc::Rc;
use uapi::OwnedFd;

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
