use crate::backend::BackendDrmLease;
use crate::backend::BackendDrmLessee;
use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::WpDrmLeaseV1Id;
use crate::wire::wp_drm_lease_v1::*;
use jay_proc::Object;
use std::cell::Cell;
use std::convert::Infallible;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct WpDrmLeaseV1Lessee {
    pub obj: Rc<WpDrmLeaseV1>,
}

impl BackendDrmLessee for WpDrmLeaseV1Lessee {
    fn created(&self, lease: Rc<dyn BackendDrmLease>) {
        if !self.obj.finished.get() {
            self.obj.send_lease_fd(lease.fd());
            self.obj.lease.set(Some(lease));
        }
    }
}

impl Drop for WpDrmLeaseV1Lessee {
    fn drop(&mut self) {
        if !self.obj.finished.get() {
            self.obj.detach();
            self.obj.send_finished();
        }
    }
}

#[derive(Object)]
#[break_loops]
pub struct WpDrmLeaseV1 {
    pub id: WpDrmLeaseV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub finished: Cell<bool>,
    pub lease: CloneCell<Option<Rc<dyn BackendDrmLease>>>,
}

impl WpDrmLeaseV1 {
    fn detach(&self) {
        self.finished.set(true);
        self.lease.take();
    }

    fn send_lease_fd(&self, fd: &Rc<OwnedFd>) {
        self.client.event(LeaseFd {
            self_id: self.id,
            leased_fd: fd.clone(),
        });
    }

    pub fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id });
    }
}

impl WpDrmLeaseV1RequestHandler for WpDrmLeaseV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self);
        Ok(())
    }
}

impl BreakLoops for WpDrmLeaseV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}
