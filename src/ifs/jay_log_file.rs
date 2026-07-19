use crate::client::Client;
use crate::client::ClientError;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::wire::JayLogFileId;
use crate::wire::jay_log_file::*;
use bstr::BStr;
use std::rc::Rc;
use thiserror::Error;

pub struct JayLogFile {
    pub id: JayLogFileId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl JayLogFile {
    pub fn new(id: JayLogFileId, client: &Rc<Client>) -> Self {
        Self {
            id,
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
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayLogFile;
    version = Version(1);
}

impl Object for JayLogFile {}

simple_add_obj!(JayLogFile);

#[derive(Debug, Error)]
pub enum JayLogFileError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayLogFileError, ClientError);
