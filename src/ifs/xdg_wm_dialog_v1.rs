use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_dialog_v1::{
            XdgDialogV1, XdgDialogV1Error,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{xdg_wm_dialog_v1::*, XdgWmDialogV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct XdgWmDialogV1Global {
    name: GlobalName,
}

impl XdgWmDialogV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XdgWmDialogV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), XdgWmDialogV1Error> {
        let obj = Rc::new(XdgWmDialogV1 {
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

global_base!(XdgWmDialogV1Global, XdgWmDialogV1, XdgWmDialogV1Error);

impl Global for XdgWmDialogV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(XdgWmDialogV1Global);

pub struct XdgWmDialogV1 {
    pub id: XdgWmDialogV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl XdgWmDialogV1RequestHandler for XdgWmDialogV1 {
    type Error = XdgWmDialogV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_xdg_dialog(&self, req: GetXdgDialog, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tl = self.client.lookup(req.toplevel)?;
        let obj = Rc::new(XdgDialogV1 {
            id: req.id,
            client: self.client.clone(),
            toplevel: tl,
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.install()?;
        Ok(())
    }
}

object_base! {
    self = XdgWmDialogV1;
    version = self.version;
}

impl Object for XdgWmDialogV1 {}

simple_add_obj!(XdgWmDialogV1);

#[derive(Debug, Error)]
pub enum XdgWmDialogV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    XdgDialogV1Error(#[from] XdgDialogV1Error),
}
efrom!(XdgWmDialogV1Error, ClientError);
