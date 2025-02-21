use {
    crate::{
        format::{formats, map_wayland_format_id},
        object::Version,
        utils::copyhashmap::CopyHashMap,
        wire::{WlShmId, wl_shm::*},
        wl_usr::{UsrCon, usr_ifs::usr_wl_shm_pool::UsrWlShmPool, usr_object::UsrObject},
    },
    std::{convert::Infallible, rc::Rc},
    uapi::OwnedFd,
};

pub struct UsrWlShm {
    pub id: WlShmId,
    pub con: Rc<UsrCon>,
    pub formats: CopyHashMap<u32, &'static crate::format::Format>,
    pub version: Version,
}

impl UsrWlShm {
    #[expect(dead_code)]
    pub fn create_pool(&self, fd: &Rc<OwnedFd>, size: i32) -> Rc<UsrWlShmPool> {
        let pool = Rc::new(UsrWlShmPool {
            id: self.con.id(),
            con: self.con.clone(),
            version: self.version,
        });
        self.con.request(CreatePool {
            self_id: self.id,
            id: pool.id,
            fd: fd.clone(),
            size,
        });
        self.con.add_object(pool.clone());
        pool
    }
}

impl WlShmEventHandler for UsrWlShm {
    type Error = Infallible;

    fn format(&self, ev: Format, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let format = map_wayland_format_id(ev.format);
        if let Some(format) = formats().get(&format) {
            self.formats.set(format.drm, *format);
        }
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlShm = WlShm;
    version = self.version;
}

impl UsrObject for UsrWlShm {
    fn destroy(&self) {
        // nothing
    }
}
