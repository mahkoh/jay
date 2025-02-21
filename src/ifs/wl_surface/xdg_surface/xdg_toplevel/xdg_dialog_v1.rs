use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel,
        leaks::Tracker,
        object::{Object, Version},
        wire::{XdgDialogV1Id, XdgToplevelId, xdg_dialog_v1::*},
    },
    std::{fmt::Debug, rc::Rc},
    thiserror::Error,
};

pub struct XdgDialogV1 {
    pub id: XdgDialogV1Id,
    pub client: Rc<Client>,
    pub toplevel: Rc<XdgToplevel>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl XdgDialogV1 {
    fn detach(&self) {
        self.toplevel.dialog.take();
    }

    pub fn install(self: &Rc<Self>) -> Result<(), XdgDialogV1Error> {
        if self.toplevel.dialog.is_some() {
            return Err(XdgDialogV1Error::AlreadyAttached(self.toplevel.id));
        }
        self.toplevel.dialog.set(Some(self.clone()));
        Ok(())
    }
}

impl XdgDialogV1RequestHandler for XdgDialogV1 {
    type Error = XdgDialogV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_modal(&self, _req: SetModal, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn unset_modal(&self, _req: UnsetModal, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

object_base! {
    self = XdgDialogV1;
    version = self.version;
}

impl Object for XdgDialogV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(XdgDialogV1);

#[derive(Debug, Error)]
pub enum XdgDialogV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Toplevel {0} already has an xdg_dialog_v1")]
    AlreadyAttached(XdgToplevelId),
}
efrom!(XdgDialogV1Error, ClientError);
