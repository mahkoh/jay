use crate::object::Version;
use crate::wire::WlShmPoolId;
use crate::wire::wl_shm_pool::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrWlShmPool {
    pub id: WlShmPoolId,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrWlShmPool {
    #[expect(dead_code)]
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
