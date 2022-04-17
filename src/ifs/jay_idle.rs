use std::time::Duration;
use thiserror::Error;
use {
    crate::{
        client::Client,
        leaks::Tracker,
        object::Object,
        wire::{jay_idle::*},
    },
    std::rc::Rc,
};
use crate::client::ClientError;
use crate::utils::buffd::{MsgParser, MsgParserError};
use crate::wire::JayIdleId;

pub struct JayIdle {
    pub id: JayIdleId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl JayIdle {
    fn send_interval(&self) {
        let to = self.client.state.idle.timeout.get();
        self.client.event(Interval {
            self_id: self.id,
            interval: to.as_secs(),
        });
    }

    fn get_status(&self, parser: MsgParser<'_, '_>) -> Result<(), JayIdleError> {
        let _req: GetStatus = self.client.parse(self, parser)?;
        self.send_interval();
        Ok(())
    }

    fn set_interval(&self, parser: MsgParser<'_, '_>) -> Result<(), JayIdleError> {
        let req: SetInterval = self.client.parse(self, parser)?;
        let interval = Duration::from_secs(req.interval);
        self.client.state.idle.set_timeout(interval);
        Ok(())
    }
}

object_base2! {
    JayIdle;

    GET_STATUS => get_status,
    SET_INTERVAL => set_interval,
}

impl Object for JayIdle {
    fn num_requests(&self) -> u32 {
        SET_INTERVAL + 1
    }
}

simple_add_obj!(JayIdle);

#[derive(Debug, Error)]
pub enum JayIdleError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayIdleError, ClientError);
efrom!(JayIdleError, MsgParserError);
