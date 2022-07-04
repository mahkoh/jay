use {
    crate::{
        wire::{wl_shm_pool::*, WlShmPoolId},
        wl_usr::{usr_ifs::usr_wl_buffer::UsrWlBuffer, usr_object::UsrObject, UsrCon},
    },
    std::{cell::Cell, rc::Rc},
    uapi::OwnedFd,
};

pub struct UsrWlShmPool {
    pub id: WlShmPoolId,
    pub con: Rc<UsrCon>,
    pub fd: Rc<OwnedFd>,
    pub size: Cell<i32>,
}

impl UsrWlShmPool {
    pub fn request_create_buffer(&self, offset: i32, buffer: &UsrWlBuffer) {
        self.con.request(CreateBuffer {
            self_id: self.id,
            id: buffer.id,
            offset,
            width: buffer.width,
            height: buffer.height,
            stride: buffer.stride.unwrap(),
            format: buffer.format.wl_id.unwrap_or(buffer.format.drm),
        });
    }

    #[allow(dead_code)]
    pub fn request_resize(&self) {
        self.con.request(Resize {
            self_id: self.id,
            size: self.size.get(),
        });
    }
}

impl Drop for UsrWlShmPool {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrWlShmPool, WlShmPool;
}

impl UsrObject for UsrWlShmPool {}
