use {
    crate::{
        object::Version,
        wire::{wl_shm_pool::*, WlShmPoolId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{convert::Infallible, rc::Rc},
};

pub struct UsrWlShmPool {
    pub id: WlShmPoolId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWlShmPool {
    #[allow(dead_code)]
    pub fn resize(&self, size: i32) {
        self.con.request(Resize {
            self_id: self.id,
            size,
        });
    }
}

impl WlShmPoolEventHandler for UsrWlShmPool {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrWlShmPool = WlShmPool;
    version = self.version;
}

impl UsrObject for UsrWlShmPool {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
