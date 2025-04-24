use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        utils::oserror::OsError,
        wire::{JayReexecId, jay_reexec::*},
    },
    std::{cell::RefCell, rc::Rc},
    thiserror::Error,
    uapi::UstrPtr,
};

pub struct JayReexec {
    pub id: JayReexecId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub args: RefCell<Vec<String>>,
}

impl JayReexec {
    fn send_failed(&self, msg: &str) {
        self.client.event(Failed {
            self_id: self.id,
            msg,
        });
    }
}

impl JayReexecRequestHandler for JayReexec {
    type Error = JayReexecError;

    fn arg(&self, req: Arg<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.args.borrow_mut().push(req.arg.to_owned());
        Ok(())
    }

    fn exec(&self, req: Exec<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let args = self.args.borrow();
        let mut args2 = UstrPtr::new();
        args2.push(req.path);
        for arg in &*args {
            args2.push(&**arg);
        }
        if let Err(e) = uapi::execvp(req.path, &args2) {
            self.send_failed(&OsError(e.0).to_string());
        }
        Ok(())
    }
}

object_base! {
    self = JayReexec;
    version = self.version;
}

impl Object for JayReexec {}

simple_add_obj!(JayReexec);

#[derive(Debug, Error)]
pub enum JayReexecError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayReexecError, ClientError);
