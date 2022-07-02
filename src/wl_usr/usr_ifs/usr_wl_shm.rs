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
};

pub struct UsrWlShm {
    pub id: WlShmId,
    pub con: Rc<UsrCon>,
    pub formats: CopyHashMap<u32, &'static crate::format::Format>,
}

impl UsrWlShm {
    pub fn request_create_pool(&self, pool: &UsrWlShmPool) {
        self.con.request(CreatePool {
            self_id: self.id,
            id: pool.id,
            fd: pool.fd.clone(),
            size: pool.size.get(),
        })
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

impl UsrObject for UsrWlShm {}
