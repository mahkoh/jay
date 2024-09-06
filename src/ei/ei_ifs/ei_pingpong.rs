use {
    crate::{
        ei::{
            ei_client::{EiClient, EiClientError},
            ei_object::{EiObject, EiVersion},
        },
        leaks::Tracker,
        wire_ei::{
            ei_pingpong::{Done, EiPingpongRequestHandler},
            EiPingpongId,
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

#[expect(dead_code)]
pub struct EiPingpong {
    pub id: EiPingpongId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
}

impl EiPingpongRequestHandler for EiPingpong {
    type Error = EiPingpongError;

    fn done(&self, _req: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

ei_object_base! {
    self = EiPingpong;
    version = self.version;
}

impl EiObject for EiPingpong {}

#[derive(Debug, Error)]
pub enum EiPingpongError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
}
efrom!(EiPingpongError, EiClientError);
