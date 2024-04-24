use {
    crate::{
        client::{Client, ClientCaps, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_security_context_v1::*, WpSecurityContextV1Id},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct WpSecurityContextV1 {
    pub id: WpSecurityContextV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub listen_fd: Rc<OwnedFd>,
    pub close_fd: Rc<OwnedFd>,
    pub sandbox_engine: RefCell<Option<String>>,
    pub app_id: RefCell<Option<String>>,
    pub instance_id: RefCell<Option<String>>,
    pub committed: Cell<bool>,
}

impl WpSecurityContextV1 {
    fn check_committed(&self) -> Result<(), WpSecurityContextV1Error> {
        if self.committed.get() {
            return Err(WpSecurityContextV1Error::Committed);
        }
        Ok(())
    }
}

impl WpSecurityContextV1RequestHandler for WpSecurityContextV1 {
    type Error = WpSecurityContextV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_sandbox_engine(
        &self,
        req: SetSandboxEngine<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.check_committed()?;
        let val = &mut *self.sandbox_engine.borrow_mut();
        if val.is_some() {
            return Err(WpSecurityContextV1Error::EnginSet);
        }
        *val = Some(req.name.to_string());
        Ok(())
    }

    fn set_app_id(&self, req: SetAppId<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.check_committed()?;
        let val = &mut *self.app_id.borrow_mut();
        if val.is_some() {
            return Err(WpSecurityContextV1Error::AppSet);
        }
        *val = Some(req.app_id.to_string());
        Ok(())
    }

    fn set_instance_id(&self, req: SetInstanceId<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.check_committed()?;
        let val = &mut *self.instance_id.borrow_mut();
        if val.is_some() {
            return Err(WpSecurityContextV1Error::InstanceSet);
        }
        *val = Some(req.instance_id.to_string());
        Ok(())
    }

    fn commit(&self, _req: Commit, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.check_committed()?;
        self.committed.set(true);
        let caps = ClientCaps::none() & self.client.bounding_caps;
        self.client.state.security_context_acceptors.spawn(
            &self.client.state,
            self.sandbox_engine.take(),
            self.app_id.take(),
            self.instance_id.take(),
            &self.listen_fd,
            &self.close_fd,
            caps,
        );
        Ok(())
    }
}

object_base! {
    self = WpSecurityContextV1;
    version = self.version;
}

impl Object for WpSecurityContextV1 {}

simple_add_obj!(WpSecurityContextV1);

#[derive(Debug, Error)]
pub enum WpSecurityContextV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The sandbox engine has already been set")]
    EnginSet,
    #[error("The app id has already been set")]
    AppSet,
    #[error("The instance id has already been set")]
    InstanceSet,
    #[error("The context has already been committed")]
    Committed,
}
efrom!(WpSecurityContextV1Error, ClientError);
