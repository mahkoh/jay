use crate::client::Client;
use crate::ifs::jay_debugfs_mount_request::JayDebugfsMountRequest;
use crate::ifs::jay_debugfs_snapshot::JayDebugfsSnapshot;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fuse::fuse_error::FuseError;
use crate::wire::JayDebugfsId;
use crate::wire::jay_debugfs::*;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[derive(Object)]
pub struct JayDebugfs {
    pub id: JayDebugfsId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayDebugfsRequestHandler for JayDebugfs {
    type Error = JayDebugfsError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn create_snapshot(&self, req: CreateSnapshot<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(JayDebugfsSnapshot {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj);
        let res = self
            .client
            .state
            .snapshot_debugfs(req.root, req.json)
            .map_err(JayDebugfsError::Snapshot);
        match res {
            Ok(fd) => obj.send_success(&fd),
            Err(e) => obj.send_failure(&ErrorFmt(e).to_string()),
        }
        Ok(())
    }

    fn mount(&self, req: Mount, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(JayDebugfsMountRequest {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            request: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj);
        let request = self.client.state.mount_debugfs(obj.clone());
        obj.request.set(Some(request));
        Ok(())
    }

    fn unmount(&self, _req: Unmount, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.state.unmount_debugfs();
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum JayDebugfsError {
    #[error("Could not create a snapshot")]
    Snapshot(#[source] FuseError),
}
