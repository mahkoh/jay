use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{jay_log_file::*, JayLogFileId},
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

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    pub fn send_path(&self, path: &BStr) {
        self.client.event(Path {
            self_id: self.id,
            path,
        });
    }
}

object_base! {
    JayLogFile, JayLogFileError;

    DESTROY => destroy,
}

impl Object for JayLogFile {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }
}

simple_add_obj!(JayLogFile);

#[derive(Debug, Error)]
pub enum JayLogFileError {
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ClientError);
efrom!(DestroyError, MsgParserError);
