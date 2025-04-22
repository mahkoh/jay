use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        leaks::Tracker,
        object::{Object, Version},
        wire::{XdgToplevelTagManagerV1Id, xdg_toplevel_tag_manager_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct XdgToplevelTagManagerV1Global {
    name: GlobalName,
}

impl XdgToplevelTagManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XdgToplevelTagManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), XdgTopleveTagManagerV1Error> {
        let obj = Rc::new(XdgToplevelTagManagerV1 {
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
    XdgToplevelTagManagerV1Global,
    XdgToplevelTagManagerV1,
    XdgTopleveTagManagerV1Error
);

impl Global for XdgToplevelTagManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(XdgToplevelTagManagerV1Global);

pub struct XdgToplevelTagManagerV1 {
    pub id: XdgToplevelTagManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl XdgToplevelTagManagerV1RequestHandler for XdgToplevelTagManagerV1 {
    type Error = XdgTopleveTagManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_toplevel_tag(
        &self,
        _req: SetToplevelTag<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_toplevel_description(
        &self,
        _req: SetToplevelDescription<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

object_base! {
    self = XdgToplevelTagManagerV1;
    version = self.version;
}

impl Object for XdgToplevelTagManagerV1 {}

simple_add_obj!(XdgToplevelTagManagerV1);

#[derive(Debug, Error)]
pub enum XdgTopleveTagManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(XdgTopleveTagManagerV1Error, ClientError);
