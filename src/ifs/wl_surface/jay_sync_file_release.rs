use crate::client::Client;
use crate::client::ClientError;
use crate::gfx_api::SyncFile;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JaySyncFileReleaseId;
use crate::wire::jay_sync_file_release::*;
use jay_proc::Object;
use std::cell::Cell;
use std::rc::Rc;
use thiserror::Error;

pub struct SyncFileRelease {
    pub release: Option<Rc<JaySyncFileRelease>>,
}

impl SyncFileRelease {
    pub fn done(&mut self, sync_file: Option<&SyncFile>) {
        if let Some(release) = self.release.take() {
            release.done(sync_file);
        }
    }
}

impl Drop for SyncFileRelease {
    fn drop(&mut self) {
        self.done(None);
    }
}

#[derive(Object)]
pub struct JaySyncFileRelease {
    id: JaySyncFileReleaseId,
    client: Rc<Client>,
    pub tracker: Tracker<Self>,
    version: Version,
    destroyed: Cell<bool>,
}

impl JaySyncFileRelease {
    pub fn new(client: &Rc<Client>, id: JaySyncFileReleaseId, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            destroyed: Cell::new(false),
        }
    }

    fn done(&self, sync_file: Option<&SyncFile>) {
        if self.destroyed.get() {
            return;
        }
        match sync_file {
            None => {
                self.client.event(ReleaseImmediate { self_id: self.id });
            }
            Some(fd) => {
                self.client.event(ReleaseAsync {
                    self_id: self.id,
                    sync_file: fd.0.clone(),
                });
            }
        }
    }
}

impl JaySyncFileReleaseRequestHandler for JaySyncFileRelease {
    type Error = JaySyncFileReleaseError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroyed.set(true);
        self.client.remove_obj(self);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum JaySyncFileReleaseError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JaySyncFileReleaseError, ClientError);
