use {
    crate::{
        client::{Client, ClientError},
        gfx_api::SyncFile,
        leaks::Tracker,
        object::{Object, Version},
        wire::{JaySyncFileReleaseId, jay_sync_file_release::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct JaySyncFileRelease {
    pub id: JaySyncFileReleaseId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub destroyed: Cell<bool>,
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

    pub fn done(&self, sync_file: Option<&SyncFile>) {
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
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JaySyncFileRelease;
    version = self.version;
}

impl Object for JaySyncFileRelease {}

simple_add_obj!(JaySyncFileRelease);

#[derive(Debug, Error)]
pub enum JaySyncFileReleaseError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JaySyncFileReleaseError, ClientError);
