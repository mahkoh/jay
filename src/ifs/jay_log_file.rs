use crate::client::Client;
use crate::client::ClientError;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayLogFileId;
use crate::wire::jay_log_file::*;
use bstr::BStr;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[derive(Object)]
pub struct JayLogFile {
    id: JayLogFileId,
    version: Version,
    client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl JayLogFile {
    pub fn new(id: JayLogFileId, version: Version, client: &Rc<Client>) -> Self {
        Self {
            id,
            version,
            client: client.clone(),
            tracker: Default::default(),
        }
    }

    pub fn send_path(&self, path: &BStr) {
        self.client.event(Path {
            self_id: self.id,
            path,
        });
    }
}

impl JayLogFileRequestHandler for JayLogFile {
    type Error = JayLogFileError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum JayLogFileError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayLogFileError, ClientError);
