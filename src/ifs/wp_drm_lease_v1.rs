use {
    crate::{
        backend::{BackendDrmLease, BackendDrmLessee},
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        utils::clonecell::CloneCell,
        wire::{WpDrmLeaseV1Id, wp_drm_lease_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    uapi::OwnedFd,
};

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
    type Error = WpDrmLeaseV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpDrmLeaseV1;
    version = self.version;
}

impl Object for WpDrmLeaseV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(WpDrmLeaseV1);

#[derive(Debug, Error)]
pub enum WpDrmLeaseV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpDrmLeaseV1Error, ClientError);
