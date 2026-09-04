use crate::client::EventFormatter;
use crate::utils::buffd::MsgFormatter;
use crate::utils::buffd::buf_out::OutBufferMeta;
use crate::wire::MAX_MESSAGE_IDS;
use crate::wire::ObjectId;
use std::collections::VecDeque;
use std::hint::cold_path;
use std::mem;
use std::rc::Rc;
use uapi::OwnedFd;

const WORD_SIZE: usize = 4;
const MAX_MESSAGE_WORDS: usize = 1024 + MAX_MESSAGE_IDS + 2;

#[derive(Default)]
pub struct SyntheticBufOut {
    buf: Vec<u32>,
    len: usize,
    meta: OutBufferMeta,
    fds: Vec<Rc<OwnedFd>>,
    fds_queue: VecDeque<Rc<OwnedFd>>,
}

impl SyntheticBufOut {
    fn prepare(&mut self, id: ObjectId, num_fds: u32) -> MsgFormatter<'_> {
        let required_len = self.len + MAX_MESSAGE_WORDS;
        if self.buf.len() < required_len {
            cold_path();
            self.buf.resize(required_len, 0);
        }
        self.buf[self.len] = num_fds;
        self.len += 1;
        self.buf[self.len] = (id.raw() >> 32) as u32;
        self.len += 1;
        let buf = uapi::as_bytes_mut(&mut self.buf[self.len..]);
        let mut fmt = MsgFormatter::new2(buf, &mut self.meta, &mut self.fds);
        fmt.wide = true;
        fmt
    }

    pub fn format<T>(&mut self, event: T)
    where
        T: EventFormatter,
    {
        let mut fmt = self.prepare(event.id(), T::NUM_FDS);
        event.format(&mut fmt);
        fmt.write_len();
        self.finish();
    }

    fn finish(&mut self) {
        self.len += self.meta.write_pos / WORD_SIZE;
        self.meta.write_pos = 0;
        while let Some(mut fds) = self.meta.fds.pop_front() {
            self.fds_queue.extend(fds.fds.drain(..));
            self.fds = fds.fds;
        }
    }

    pub fn take(&mut self) -> (&[u32], &mut VecDeque<Rc<OwnedFd>>) {
        let len = mem::take(&mut self.len);
        (&self.buf[..len], &mut self.fds_queue)
    }
}
