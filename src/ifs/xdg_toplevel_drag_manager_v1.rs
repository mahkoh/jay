use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_drag_v1::XdgToplevelDragV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{xdg_toplevel_drag_manager_v1::*, XdgToplevelDragManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct XdgToplevelDragManagerV1Global {
    pub name: GlobalName,
}

impl XdgToplevelDragManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XdgToplevelDragManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), XdgToplevelDragManagerV1Error> {
        let mgr = Rc::new(XdgToplevelDragManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, mgr);
        client.add_client_obj(&mgr)?;
        Ok(())
    }
}

global_base!(
    XdgToplevelDragManagerV1Global,
    XdgToplevelDragManagerV1,
    XdgToplevelDragManagerV1Error
);

simple_add_global!(XdgToplevelDragManagerV1Global);

impl Global for XdgToplevelDragManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        2
    }
}

pub struct XdgToplevelDragManagerV1 {
    pub id: XdgToplevelDragManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl XdgToplevelDragManagerV1RequestHandler for XdgToplevelDragManagerV1 {
    type Error = XdgToplevelDragManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_xdg_toplevel_drag(
        &self,
        req: GetXdgToplevelDrag,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let source = self.client.lookup(req.data_source)?;
        if source.data.was_used() {
            return Err(XdgToplevelDragManagerV1Error::AlreadyUsed);
        }
        if source.toplevel_drag.get().is_some() {
            return Err(XdgToplevelDragManagerV1Error::HasDrag);
        }
        let drag = Rc::new(XdgToplevelDragV1::new(req.id, &source, self.version));
        track!(&self.client, drag);
        self.client.add_client_obj(&drag)?;
        source.toplevel_drag.set(Some(drag));
        Ok(())
    }
}

object_base! {
    self = XdgToplevelDragManagerV1;
    version = self.version;
}

impl Object for XdgToplevelDragManagerV1 {}

simple_add_obj!(XdgToplevelDragManagerV1);

#[derive(Debug, Error)]
pub enum XdgToplevelDragManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The data source has already been used")]
    AlreadyUsed,
    #[error("The source already has a drag object")]
    HasDrag,
}
efrom!(XdgToplevelDragManagerV1Error, ClientError);
