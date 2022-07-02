#![allow(non_upper_case_globals)]

use {
    crate::{
        pipewire::{
            pw_con::PwConData,
            pw_mem::{PwMem, PwMemType},
            pw_object::{PwObject, PwObjectData},
            pw_parser::{PwParser, PwParserError},
            pw_pod::{SPA_DATA_DmaBuf, SPA_DATA_MemFd, SpaDataType},
        },
        utils::bitflags::BitflagsExt,
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct PwCore {
    pub data: PwObjectData,
    pub con: Rc<PwConData>,
}

pw_opcodes! {
    PwCoreMethods;

    Hello = 1,
    Sync = 2,
    Pong = 3,
    Error = 4,
    GetRegistry = 5,
    CreateObject = 6,
    Destroy = 7,
}

pw_opcodes! {
    PwCoreEvents;

    Info = 0,
    Done = 1,
    Ping = 2,
    Error = 3,
    RemoveId = 4,
    BoundId = 5,
    AddMem = 6,
    RemoveMem = 7,
}

pub const PW_CORE_VERSION: i32 = 3;

impl PwCore {
    pub fn handle_info(&self, mut p1: PwParser<'_>) -> Result<(), PwCoreError> {
        let s1 = p1.read_struct()?;
        let mut p2 = s1.fields;
        let id = p2.read_int()?;
        let cookie = p2.read_int()?;
        let user_name = p2.read_string()?;
        let host_name = p2.read_string()?;
        let version_name = p2.read_string()?;
        let name = p2.read_string()?;
        let change_mask = p2.read_long()?;
        let dict = p2.read_dict_struct()?;
        log::info!("info: id={id}, cookie={cookie}, user_name={user_name}, host_name={host_name}, version_name={version_name}, name={name}, change_mask={change_mask}");
        log::info!("dict: {:#?}", dict);
        Ok(())
    }

    pub fn handle_done(&self, mut p1: PwParser<'_>) -> Result<(), PwCoreError> {
        let s1 = p1.read_struct()?;
        let mut p2 = s1.fields;
        let id = p2.read_uint()?;
        let seq = p2.read_uint()?;
        if let Some(obj) = self.con.objects.get(&id) {
            if obj.data().sync_id.get() <= seq {
                obj.data().sync_id.set(seq);
                obj.done();
            }
        }
        Ok(())
    }

    pub fn handle_ping(&self, mut p1: PwParser<'_>) -> Result<(), PwCoreError> {
        let s1 = p1.read_struct()?;
        let mut p2 = s1.fields;
        let id = p2.read_int()?;
        let seq = p2.read_int()?;
        self.con.send(self, PwCoreMethods::Pong, |f| {
            f.write_struct(|f| {
                f.write_int(id);
                f.write_int(seq);
            });
        });
        Ok(())
    }

    pub fn handle_error(&self, mut p1: PwParser<'_>) -> Result<(), PwCoreError> {
        let s1 = p1.read_struct()?;
        let mut p2 = s1.fields;
        let id = p2.read_int()?;
        let seq = p2.read_int()?;
        let res = p2.read_int()?;
        let error = p2.read_string()?;
        log::info!("error: id={id}, seq={seq}, res={res}, error={error}");
        Ok(())
    }

    pub fn handle_remove_id(&self, mut p1: PwParser<'_>) -> Result<(), PwCoreError> {
        let s1 = p1.read_struct()?;
        let mut p2 = s1.fields;
        let id = p2.read_uint()?;
        self.con.objects.remove(&id);
        self.con.ids.borrow_mut().release(id);
        Ok(())
    }

    pub fn handle_bound_id(&self, mut p1: PwParser<'_>) -> Result<(), PwCoreError> {
        let s1 = p1.read_struct()?;
        let mut p2 = s1.fields;
        let id = p2.read_uint()?;
        let bound_id = p2.read_uint()?;
        if let Some(obj) = self.con.objects.get(&id) {
            obj.data().bound_id.set(Some(bound_id));
            obj.bound_id(bound_id);
        }
        Ok(())
    }

    pub fn handle_add_mem(&self, mut p1: PwParser<'_>) -> Result<(), PwCoreError> {
        let s1 = p1.read_struct()?;
        let mut p2 = s1.fields;
        let id = p2.read_uint()?;
        let ty = SpaDataType(p2.read_id()?);
        let fd = p2.read_fd()?;
        let flags = p2.read_int()?;
        let read = flags.contains(1);
        let write = flags.contains(2);
        let ty = match ty {
            SPA_DATA_MemFd => PwMemType::MemFd,
            SPA_DATA_DmaBuf => PwMemType::DmaBuf,
            _ => {
                log::error!("Ignoring unknown mem type {:?}", ty);
                return Ok(());
            }
        };
        self.con.mem.mems.set(
            id,
            Rc::new(PwMem {
                ty,
                read,
                write,
                fd,
            }),
        );
        Ok(())
    }

    pub fn handle_remove_mem(&self, mut p1: PwParser<'_>) -> Result<(), PwCoreError> {
        let s1 = p1.read_struct()?;
        let mut p2 = s1.fields;
        let id = p2.read_uint()?;
        self.con.mem.mems.remove(&id);
        Ok(())
    }
}

pw_object_base! {
    PwCore, "core", PwCoreEvents;

    Info => handle_info,
    Done => handle_done,
    Ping => handle_ping,
    Error => handle_error,
    RemoveId => handle_remove_id,
    BoundId => handle_bound_id,
    AddMem => handle_add_mem,
    RemoveMem => handle_remove_mem,
}

impl PwObject for PwCore {}

#[derive(Debug, Error)]
pub enum PwCoreError {
    #[error(transparent)]
    PwParserError(#[from] PwParserError),
}
