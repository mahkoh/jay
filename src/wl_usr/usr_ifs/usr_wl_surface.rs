use {
    crate::{
        utils::clonecell::CloneCell,
        wire::{wl_surface::*, WlSurfaceId},
        wl_usr::{usr_ifs::usr_wl_buffer::UsrWlBuffer, usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrWlSurface {
    pub id: WlSurfaceId,
    pub con: Rc<UsrCon>,
}

impl UsrWlSurface {
    pub fn request_attach(&self, buffer: &UsrWlBuffer) {
        self.con.request(Attach {
            self_id: self.id,
            buffer: buffer.id,
            x: 0,
            y: 0,
        });
    }

    pub fn request_commit(&self) {
        self.con.request(Commit { self_id: self.id });
    }
}

impl Drop for UsrWlSurface {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrWlSurface, WlSurface;
}

impl UsrObject for UsrWlSurface {}
