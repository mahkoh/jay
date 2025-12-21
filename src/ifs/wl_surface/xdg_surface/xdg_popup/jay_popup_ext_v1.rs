use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::xdg_surface::{xdg_popup::XdgPopup, xdg_toplevel::map_resize_edges},
        leaks::Tracker,
        object::{Object, Version},
        wire::{JayPopupExtV1Id, jay_popup_ext_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayPopupExtV1 {
    id: JayPopupExtV1Id,
    client: Rc<Client>,
    pub tracker: Tracker<Self>,
    version: Version,
    popup: Rc<XdgPopup>,
}

impl JayPopupExtV1 {
    pub fn new(
        id: JayPopupExtV1Id,
        client: &Rc<Client>,
        version: Version,
        popup: &Rc<XdgPopup>,
    ) -> Self {
        Self {
            id,
            tracker: Default::default(),
            version,
            client: client.clone(),
            popup: popup.clone(),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), JayPopupExtV1Error> {
        if self.popup.jay_popup_ext.is_some() {
            return Err(JayPopupExtV1Error::HasExt);
        }
        self.popup.jay_popup_ext.set(Some(self.clone()));
        Ok(())
    }
}

impl JayPopupExtV1RequestHandler for JayPopupExtV1 {
    type Error = JayPopupExtV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.popup.jay_popup_ext.take();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn move_(&self, req: Move, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let Some(serial) = self.client.map_serial(req.serial) else {
            return Ok(());
        };
        if self.popup.seat_state.pointer_not_inside(&seat.global) {
            return Ok(());
        }
        seat.global.start_popup_move(&self.popup, serial);
        Ok(())
    }

    fn resize(&self, req: Resize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(edges) = map_resize_edges(req.edges) else {
            return Err(JayPopupExtV1Error::UnknownResizeEdges(req.edges));
        };
        let seat = self.client.lookup(req.seat)?;
        let Some(serial) = self.client.map_serial(req.serial) else {
            return Ok(());
        };
        if self.popup.seat_state.pointer_not_inside(&seat.global) {
            return Ok(());
        }
        seat.global.start_popup_resize(&self.popup, edges, serial);
        Ok(())
    }
}

object_base! {
    self = JayPopupExtV1;
    version = self.version;
}

impl Object for JayPopupExtV1 {}

simple_add_obj!(JayPopupExtV1);

#[derive(Debug, Error)]
pub enum JayPopupExtV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The xdg_popup already has a jay_popup_ext_v1 extension")]
    HasExt,
    #[error("The resize edge {0} is unknown")]
    UnknownResizeEdges(u32),
}
efrom!(JayPopupExtV1Error, ClientError);
