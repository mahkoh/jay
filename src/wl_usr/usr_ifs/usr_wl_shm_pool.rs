use {
    crate::{
        wire::{wl_shm_pool::*, WlShmPoolId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
};

pub struct UsrWlShmPool {
    pub id: WlShmPoolId,
    pub con: Rc<UsrCon>,
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

usr_object_base! {
    UsrWlShmPool, WlShmPool;
}

impl UsrObject for UsrWlShmPool {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
