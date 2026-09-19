use crate::client::Client;
use crate::dfs::DfsMounter;
use crate::dfs::PendingDfsMount;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::wire::JayDebugfsMountRequestId;
use crate::wire::jay_debugfs_mount_request::*;
use jay_proc::Object;
use std::cell::Cell;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct JayDebugfsMountRequest {
    pub id: JayDebugfsMountRequestId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub request: Cell<Option<PendingDfsMount>>,
}

impl JayDebugfsMountRequest {
    fn detach(&self) {
        self.request.take();
    }
}

impl JayDebugfsMountRequestRequestHandler for JayDebugfsMountRequest {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self);
        Ok(())
    }
}

impl DfsMounter for JayDebugfsMountRequest {
    fn success(self: Rc<Self>, path: &str) {
        self.detach();
        self.client.event(Success {
            self_id: self.id,
            path,
        });
    }

    fn failure(self: Rc<Self>) {
        self.detach();
        self.client.event(Failure { self_id: self.id });
    }
}

impl BreakLoops for JayDebugfsMountRequest {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}
