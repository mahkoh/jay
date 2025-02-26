use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{JayColorManagementId, jay_color_management::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayColorManagement {
    pub id: JayColorManagementId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayColorManagement {
    fn send_enabled(&self) {
        self.client.event(Enabled {
            self_id: self.id,
            enabled: self.client.state.color_management_enabled.get() as u32,
        });
    }
}

impl JayColorManagementRequestHandler for JayColorManagement {
    type Error = JayColorManagementError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get(&self, _req: Get, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.send_enabled();
        Ok(())
    }

    fn set_enabled(&self, req: SetEnabled, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client
            .state
            .color_management_enabled
            .set(req.enabled != 0);
        Ok(())
    }
}

object_base! {
    self = JayColorManagement;
    version = self.version;
}

impl Object for JayColorManagement {}

simple_add_obj!(JayColorManagement);

#[derive(Debug, Error)]
pub enum JayColorManagementError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayColorManagementError, ClientError);
