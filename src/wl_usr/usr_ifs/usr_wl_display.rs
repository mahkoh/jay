use {
    crate::{
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_display::*, WlDisplayId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrWlDisplay {
    pub id: WlDisplayId,
    pub con: Rc<UsrCon>,
}

impl UsrWlDisplay {
    fn error(&self, parser: MsgParser<'_, '_>) -> Result<(), UsrWlDisplayError> {
        let ev: Error = self.con.parse(self, parser)?;
        Err(UsrWlDisplayError::ServerError(ev.message.to_owned()))
    }

    fn delete_id(&self, parser: MsgParser<'_, '_>) -> Result<(), UsrWlDisplayError> {
        let ev: DeleteId = self.con.parse(self, parser)?;
        self.con.release_id(ev.id);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UsrWlDisplayError {
    #[error("The server emitted an error: {0}")]
    ServerError(String),
    #[error(transparent)]
    MsgParserError(#[from] MsgParserError),
}

usr_object_base! {
    UsrWlDisplay, WlDisplay;

    ERROR => error,
    DELETE_ID => delete_id,
}

impl UsrObject for UsrWlDisplay {}
