use std::rc::Rc;
use crate::utils::buffd::{MsgParser, MsgParserError};
use crate::utils::clonecell::CloneCell;
use crate::wire::ZwlrLayerSurfaceV1Id;
use crate::wire::zwlr_layer_surface_v1::*;
use crate::wl_usr::usr_object::UsrObject;
use crate::wl_usr::UsrCon;

pub struct UsrWlrLayerSurface {
    pub id: ZwlrLayerSurfaceV1Id,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrWlrLayerSurfaceOwner>>>,
}

pub trait UsrWlrLayerSurfaceOwner {
    fn configure(&self, ev: &Configure) { let _ = ev; }

    fn closed(&self) { }
}

impl UsrWlrLayerSurface {
    #[allow(dead_code)]
    pub fn request_set_size(&self, width: i32, height: i32) {
        self.con.request(SetSize {
            self_id: self.id,
            width: width as _,
            height: height as _,
        });
    }

    #[allow(dead_code)]
    pub fn request_set_keyboard_interactivity(&self, ki: u32) {
        self.con.request(SetKeyboardInteractivity {
            self_id: self.id,
            keyboard_interactivity: ki,
        });
    }

    #[allow(dead_code)]
    pub fn request_set_layer(&self, layer: u32) {
        self.con.request(SetLayer {
            self_id: self.id,
            layer,
        });
    }

    fn configure(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Configure = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.configure(&ev);
        }
        self.con.request(AckConfigure {
            self_id: self.id,
            serial: ev.serial,
        });
        Ok(())
    }

    fn closed(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Closed = self.con.parse(self, parser)?;
        if let Some(owner) = self.owner.get() {
            owner.closed();
        }
        Ok(())
    }
}

impl Drop for UsrWlrLayerSurface {
    fn drop(&mut self) {
        self.con.request(Destroy {
            self_id: self.id,
        });
    }
}

usr_object_base! {
    UsrWlrLayerSurface, ZwlrLayerSurfaceV1;

    CONFIGURE => configure,
    CLOSED => closed,
}

impl UsrObject for UsrWlrLayerSurface {
    fn break_loops(&self) {
        self.owner.take();
    }
}
