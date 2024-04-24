use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wp_security_context_v1::WpSecurityContextV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_security_context_manager_v1::*, WpSecurityContextManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpSecurityContextManagerV1Global {
    pub name: GlobalName,
}

impl WpSecurityContextManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpSecurityContextManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpSecurityContextManagerV1Error> {
        let obj = Rc::new(WpSecurityContextManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    WpSecurityContextManagerV1Global,
    WpSecurityContextManagerV1,
    WpSecurityContextManagerV1Error
);

impl Global for WpSecurityContextManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpSecurityContextManagerV1Global);

pub struct WpSecurityContextManagerV1 {
    pub id: WpSecurityContextManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpSecurityContextManagerV1RequestHandler for WpSecurityContextManagerV1 {
    type Error = WpSecurityContextManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn create_listener(&self, req: CreateListener, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(WpSecurityContextV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            listen_fd: req.listen_fd,
            close_fd: req.close_fd,
            sandbox_engine: Default::default(),
            app_id: Default::default(),
            instance_id: Default::default(),
            committed: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }
}

object_base! {
    self = WpSecurityContextManagerV1;
    version = self.version;
}

impl Object for WpSecurityContextManagerV1 {}

simple_add_obj!(WpSecurityContextManagerV1);

#[derive(Debug, Error)]
pub enum WpSecurityContextManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpSecurityContextManagerV1Error, ClientError);
