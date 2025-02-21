use {
    crate::{
        object::Version,
        wire::{WlDisplayId, wl_display::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::rc::Rc,
};

pub struct UsrWlDisplay {
    pub id: WlDisplayId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl WlDisplayEventHandler for UsrWlDisplay {
    type Error = UsrWlDisplayError;

    fn error(&self, ev: Error<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Err(UsrWlDisplayError::ServerError(ev.message.to_owned()))
    }

    fn delete_id(&self, ev: DeleteId, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.con.release_id(ev.id);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UsrWlDisplayError {
    #[error("The server emitted an error: {0}")]
    ServerError(String),
}

usr_object_base! {
    self = UsrWlDisplay = WlDisplay;
    version = self.version;
}

impl UsrObject for UsrWlDisplay {
    fn destroy(&self) {
        // nothing
    }
}
