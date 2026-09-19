use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayDebugfsSnapshotId;
use crate::wire::jay_debugfs_snapshot::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;
use uapi::OwnedFd;

#[derive(Object)]
pub struct JayDebugfsSnapshot {
    pub id: JayDebugfsSnapshotId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayDebugfsSnapshot {
    pub fn send_success(&self, fd: &Rc<OwnedFd>) {
        self.client.event(Success {
            self_id: self.id,
            fd: fd.clone(),
        });
    }

    pub fn send_failure(&self, msg: &str) {
        self.client.event(Failure {
            self_id: self.id,
            msg,
        });
    }
}

impl JayDebugfsSnapshotRequestHandler for JayDebugfsSnapshot {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}
