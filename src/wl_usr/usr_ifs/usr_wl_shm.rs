use {
    crate::{
        format::{formats, map_wayland_format_id},
        utils::{
            buffd::{MsgParser, MsgParserError},
            copyhashmap::CopyHashMap,
        },
        wire::{wl_shm::*, WlShmId},
        wl_usr::{usr_ifs::usr_wl_shm_pool::UsrWlShmPool, usr_object::UsrObject, UsrCon},
    },
    std::rc::Rc,
    uapi::OwnedFd,
};

pub struct UsrWlShm {
    pub id: WlShmId,
    pub con: Rc<UsrCon>,
    pub formats: CopyHashMap<u32, &'static crate::format::Format>,
}

impl UsrWlShm {
    #[allow(dead_code)]
    pub fn create_pool(&self, fd: &Rc<OwnedFd>, size: i32) -> Rc<UsrWlShmPool> {
        let pool = Rc::new(UsrWlShmPool {
            id: self.con.id(),
            con: self.con.clone(),
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

    fn format(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let ev: Format = self.con.parse(self, parser)?;
        let format = map_wayland_format_id(ev.format);
        if let Some(format) = formats().get(&format) {
            self.formats.set(format.drm, *format);
        }
        Ok(())
    }
}

usr_object_base! {
    UsrWlShm, WlShm;

    FORMAT => format,
}

impl UsrObject for UsrWlShm {
    fn destroy(&self) {
        // nothing
    }
}
