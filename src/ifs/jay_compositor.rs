use crate::client::{Client, ClientError};
use crate::globals::{Global, GlobalName};
use crate::ifs::jay_log_file::JayLogFile;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::utils::buffd::{MsgParser, MsgParserError};
use crate::wire::jay_compositor::*;
use crate::wire::JayCompositorId;
use std::rc::Rc;
use thiserror::Error;

pub struct JayCompositorGlobal {
    name: GlobalName,
}

impl JayCompositorGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: JayCompositorId,
        client: &Rc<Client>,
        _version: u32,
    ) -> Result<(), JayCompositorError> {
        let obj = Rc::new(JayCompositor {
            id,
            client: client.clone(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(JayCompositorGlobal, JayCompositor, JayCompositorError);

impl Global for JayCompositorGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn secure(&self) -> bool {
        true
    }
}

simple_add_global!(JayCompositorGlobal);

pub struct JayCompositor {
    id: JayCompositorId,
    client: Rc<Client>,
    tracker: Tracker<Self>,
}

impl JayCompositor {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_log_file(&self, parser: MsgParser<'_, '_>) -> Result<(), GetLogFileError> {
        let req: GetLogFile = self.client.parse(self, parser)?;
        let log_file = Rc::new(JayLogFile::new(req.id, &self.client));
        track!(self.client, log_file);
        self.client.add_client_obj(&log_file)?;
        log_file.send_path(self.client.state.logger.path());
        Ok(())
    }
}

object_base! {
    JayCompositor, JayCompositorError;

    DESTROY => destroy,
    GET_LOG_FILE => get_log_file,
}

impl Object for JayCompositor {
    fn num_requests(&self) -> u32 {
        GET_LOG_FILE + 1
    }
}

simple_add_obj!(JayCompositor);

#[derive(Debug, Error)]
pub enum JayCompositorError {
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `get_log_file` request")]
    GetLogFileError(#[from] GetLogFileError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayCompositorError, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ClientError);
efrom!(DestroyError, MsgParserError);

#[derive(Debug, Error)]
pub enum GetLogFileError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetLogFileError, ClientError);
efrom!(GetLogFileError, MsgParserError);
