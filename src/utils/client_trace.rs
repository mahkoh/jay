use crate::fixed::Fixed;
use crate::io_uring::IoUring;
use crate::io_uring::IoUringError;
use crate::utils::client_trace::generated::ARGS;
use crate::utils::client_trace::generated::ClientTraceArray;
use crate::utils::client_trace::generated::ClientTracePod;
use crate::utils::client_trace::generated::DEFS;
use crate::utils::client_trace::generated::READERS;
use crate::utils::client_trace::private::ClientTraceMessagePriv;
use crate::utils::cross_process_ring_buffer::Cprb;
use crate::utils::cross_process_ring_buffer::CprbError;
use crate::utils::cross_process_ring_buffer::CprbMsgRead;
use crate::utils::cross_process_ring_buffer::CprbRead;
use crate::utils::cross_process_ring_buffer::CprbReadAvailable;
use crate::utils::cross_process_ring_buffer::CprbWrite;
use crate::utils::fx_hash::FHashMap;
use crate::utils::maybe_uninit::MaybeUninitSliceExt2;
use crate::utils::ptr_ext::MutPtrExt;
use crate::utils::ptr_ext::PtrExt;
use crate::utils::str_table::StrAccess;
use crate::wire::ObjectId;
use generated::MAX_ARGS;
use hashbrown::hash_map::Entry;
use std::array;
use std::mem::MaybeUninit;
use std::rc::Rc;
use uapi::OwnedFd;

mod fmt;
mod helpers;
#[cfg(test)]
mod tests;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/client_trace/mod.rs"));
}

const NUM_SLOTS: usize = 1024;
const WORD_SIZE: usize = size_of::<u32>();
const MAX_MESSAGE_SIZE: usize = 4096;
const MAX_MESSAGE_WORDS: usize = MAX_MESSAGE_SIZE / WORD_SIZE;
const MAX_MESSAGE_WORDS_64: u64 = MAX_MESSAGE_WORDS as u64;
const DATA_WORDS: usize = NUM_SLOTS * MAX_MESSAGE_WORDS;
const DATA_WORDS_64: u64 = DATA_WORDS as u64;

pub trait ClientTraceMessage: ClientTraceMessagePriv {}

pub struct ClientTraceMessageDef {
    pub is_request: bool,
    pub has_ids: bool,
    pub interface: StrAccess,
    pub message: StrAccess,
    args: (u16, u16),
}

pub struct ClientTraceArgDef {
    pub name: StrAccess,
    pub interface: Option<StrAccess>,
}

pub struct ClientTraceArg<'a> {
    pub def: &'static ClientTraceArgDef,
    pub val: ClientTraceArgVal<'a>,
}

#[derive(Copy, Clone)]
pub enum ClientTraceArgVal<'a> {
    Id(u64),
    U32(u32),
    I32(i32),
    U64(u64),
    Str(Option<&'a [u8]>),
    Fixed(Fixed),
    Bool(bool),
    Fd,
    Array(ClientTraceArray<'a>),
    Pod(ClientTracePod),
}

static_assertions::assert_impl_all!(ClientTraceArgVal: Copy);

mod private {
    use crate::utils::client_trace::MAX_MESSAGE_WORDS;

    pub trait ClientTraceMessagePriv {
        fn write(&self, id: &mut u32, data: &mut [u32; MAX_MESSAGE_WORDS]) -> Option<usize>;
    }
}

impl<T> ClientTraceMessage for T where T: ClientTraceMessagePriv {}

struct ClientTraceCprb;

impl Cprb for ClientTraceCprb {
    type Slot = Slot;
    type Data = [u32; DATA_WORDS];
}

pub struct ClientTraceWrite {
    write: CprbWrite<ClientTraceCprb, NUM_SLOTS>,
}

pub struct ClientTraceMsg<'a> {
    _msg: CprbMsgRead<'a, ClientTraceCprb, NUM_SLOTS>,
    pub us: u64,
    pub def: &'static ClientTraceMessageDef,
    pub obj: u64,
    pub args: &'a mut [ClientTraceArg<'a>],
}

pub struct ClientTraceRead {
    read: CprbRead<ClientTraceCprb, NUM_SLOTS>,
    storage: [MaybeUninit<ClientTraceArg<'static>>; MAX_ARGS],
    id_map: IdMap,
}

#[derive(Clone)]
pub struct ClientTraceReadAvailable {
    available: CprbReadAvailable<ClientTraceCprb, NUM_SLOTS>,
}

#[derive(Copy, Clone)]
enum Slot {
    DeleteId(DeleteIdSlot),
    Message(MessageSlot),
}

static_assertions::assert_impl_all!(Slot: Copy);

#[derive(Copy, Clone)]
struct DeleteIdSlot {
    obj: ObjectId,
}

#[derive(Copy, Clone)]
struct MessageSlot {
    message: u32,
    obj: ObjectId,
    offset: u32,
    us: u64,
}

#[derive(Default)]
struct IdMap {
    map: FHashMap<u64, u64>,
    next: u64,
    raw_ids: bool,
}

pub struct ClientTraceEvent<'a> {
    pub missed: u64,
    pub msg: Option<ClientTraceMsg<'a>>,
}

pub type Reader =
    for<'a, 'b> unsafe fn(*mut u32, &'b mut [MaybeUninit<ClientTraceArg<'a>>; MAX_ARGS]);

impl IdMap {
    fn get(&mut self, n: u64) -> u64 {
        if self.raw_ids {
            return n;
        }
        match self.map.entry(n) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                let v = self.next;
                self.next += 1;
                e.insert(v);
                v
            }
        }
    }

    fn remove(&mut self, n: ObjectId) {
        self.map.remove(&n.raw());
    }
}

impl ClientTraceWrite {
    pub fn new(ring: &Rc<IoUring>) -> Result<Self, CprbError> {
        Ok(Self {
            write: CprbWrite::new(ring)?,
        })
    }

    pub unsafe fn memfd(&self) -> &Rc<OwnedFd> {
        self.write.memfd()
    }

    pub fn write_delete_id(&self, obj: ObjectId) {
        let Some(msg) = self.write.acquire() else {
            return;
        };
        let slot = unsafe { msg.slot.deref_mut() };
        *slot = Slot::DeleteId(DeleteIdSlot { obj });
        self.write.commit(msg.write);
    }

    pub fn write_msg(&self, id: ObjectId, now_us: u64, msg: &dyn ClientTraceMessage) {
        let Some(write) = self.write.acquire() else {
            return;
        };
        let write_offset = write.write;
        let read_offset = write.read;
        let used = write_offset - read_offset;
        let free = DATA_WORDS_64 - used;
        if free < MAX_MESSAGE_WORDS_64 {
            return;
        }
        let lo = (write_offset % DATA_WORDS_64) as usize;
        let data = unsafe {
            self.write
                .data()
                .cast::<u32>()
                .add(lo)
                .cast::<[u32; MAX_MESSAGE_WORDS]>()
                .deref_mut()
        };
        let mut msg_id = 0;
        let Some(used) = msg.write(&mut msg_id, data) else {
            self.warn_msg(msg_id);
            return;
        };
        let slot = unsafe { write.slot.deref_mut() };
        *slot = Slot::Message(MessageSlot {
            message: msg_id,
            obj: id,
            offset: lo as u32,
            us: now_us,
        });
        let mut offset = write_offset + used as u64;
        let next_wrap = offset.next_multiple_of(DATA_WORDS_64);
        if next_wrap - offset < MAX_MESSAGE_WORDS_64 {
            offset = next_wrap;
        }
        self.write.commit(offset);
    }

    #[cold]
    fn warn_msg(&self, msg_id: u32) {
        let def = &DEFS[msg_id as usize];
        let interface = def.interface;
        let message = def.message;
        log::warn!(
            "Message of type {interface}.{message} does not fit into {MAX_MESSAGE_SIZE} bytes"
        );
    }
}

impl ClientTraceRead {
    pub unsafe fn new(
        ring: &Rc<IoUring>,
        memfd: &Rc<OwnedFd>,
        raw_ids: bool,
    ) -> Result<ClientTraceRead, CprbError> {
        let mut id_map = IdMap::default();
        id_map.raw_ids = raw_ids;
        id_map.get(0);
        Ok(ClientTraceRead {
            read: CprbRead::new(ring, memfd)?,
            storage: array::from_fn(|_| MaybeUninit::uninit()),
            id_map,
        })
    }

    pub fn available(&self) -> ClientTraceReadAvailable {
        ClientTraceReadAvailable {
            available: self.read.available(),
        }
    }

    pub fn try_read(&mut self) -> Option<ClientTraceEvent<'_>> {
        let data = self.read.data();
        let msg = self.read.acquire()?;
        let missed = msg.missed;
        let slot = unsafe { msg.slot.deref() };
        let id_map = &mut self.id_map;
        let msg = match slot {
            Slot::DeleteId(slot) => {
                id_map.remove(slot.obj);
                None
            }
            Slot::Message(slot) => {
                let def_idx = slot.message as usize;
                let reader = unsafe { READERS.get_unchecked(def_idx) };
                let args = self.storage.cast_mut();
                unsafe {
                    reader(data.cast::<u32>().add(slot.offset as usize), args);
                }
                let obj = id_map.get(slot.obj.raw());
                let def = unsafe { DEFS.get_unchecked(def_idx) };
                let args = unsafe {
                    let lo = def.args.0 as usize;
                    let hi = def.args.1 as usize;
                    let arg_defs = ARGS.get_unchecked(lo..hi);
                    for (idx, def) in arg_defs.iter().enumerate() {
                        (&raw mut (*args.get_unchecked_mut(idx).as_mut_ptr()).def).write(def);
                    }
                    args.get_unchecked_mut(..arg_defs.len()).assume_init_mut()
                };
                if def.has_ids && !id_map.raw_ids {
                    for arg in &mut *args {
                        if let ClientTraceArgVal::Id(id) = &mut arg.val {
                            *id = id_map.get(*id);
                        }
                    }
                }
                Some(ClientTraceMsg {
                    _msg: msg,
                    us: slot.us,
                    def,
                    obj,
                    args,
                })
            }
        };
        Some(ClientTraceEvent { missed, msg })
    }
}

impl ClientTraceReadAvailable {
    pub async fn available(&self) -> Result<(), IoUringError> {
        self.available.available().await
    }
}
