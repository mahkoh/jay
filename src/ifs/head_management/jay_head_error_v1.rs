use {
    crate::{
        client::{Client, ClientError},
        ifs::head_management::HeadTransactionError,
        leaks::Tracker,
        object::{Object, Version},
        utils::errorfmt::ErrorFmt,
        wire::{JayHeadErrorV1Id, jay_head_error_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayHeadErrorV1 {
    pub(super) id: JayHeadErrorV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) version: Version,
    pub(super) error: Rc<HeadTransactionError>,
}

impl JayHeadErrorV1 {
    pub fn send(&self) {
        let msg = ErrorFmt(&*self.error).to_string();
        self.send_message(&msg);
        match &*self.error {
            HeadTransactionError::HeadRemoved(_) | HeadTransactionError::MonitorChanged(_) => {
                self.send_out_of_date();
            }
            HeadTransactionError::AlreadyFailed => {
                self.send_already_failed();
            }
            HeadTransactionError::Backend(_) => {}
        }
        self.client.event(Done { self_id: self.id });
    }

    fn send_message(&self, message: &str) {
        self.client.event(Message {
            self_id: self.id,
            msg: message,
        });
    }

    fn send_out_of_date(&self) {
        self.client.event(OutOfDate { self_id: self.id });
    }

    fn send_already_failed(&self) {
        self.client.event(AlreadyFailed { self_id: self.id });
    }
}

impl JayHeadErrorV1RequestHandler for JayHeadErrorV1 {
    type Error = JayHeadErrorV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayHeadErrorV1;
    version = self.version;
}

impl Object for JayHeadErrorV1 {}

dedicated_add_obj!(JayHeadErrorV1, JayHeadErrorV1Id, jay_head_errors);

#[derive(Debug, Error)]
pub enum JayHeadErrorV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayHeadErrorV1Error, ClientError);
