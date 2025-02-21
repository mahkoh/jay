use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{JayLogFileId, jay_log_file::*},
    },
    bstr::BStr,
    std::rc::Rc,
    thiserror::Error,
};

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
