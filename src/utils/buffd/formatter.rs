use crate::object::ObjectId;
use crate::utils::buffd::buf_out::{BufFdOut, MsgFds};
use std::mem;
use std::mem::MaybeUninit;
use uapi::OwnedFd;

pub struct MsgFormatter<'a> {
    buf: &'a mut BufFdOut,
    pos: usize,
    fds: Vec<OwnedFd>,
}

impl<'a> MsgFormatter<'a> {
    pub fn new(buf: &'a mut BufFdOut) -> Self {
        Self {
            pos: buf.out_pos,
            buf,
            fds: vec![],
        }
    }

    pub fn int(&mut self, int: i32) -> &mut Self {
        self.buf.write(uapi::as_maybe_uninit_bytes(&int));
        self
    }

    pub fn uint(&mut self, int: u32) -> &mut Self {
        self.buf.write(uapi::as_maybe_uninit_bytes(&int));
        self
    }

    pub fn fixed(&mut self, fixed: f64) -> &mut Self {
        let int = (fixed * 256.0) as i32;
        self.buf.write(uapi::as_maybe_uninit_bytes(&int));
        self
    }

    pub fn string(&mut self, s: &str) -> &mut Self {
        let len = s.len() + 1;
        let cap = (len + 3) & !3;
        self.uint(len as u32);
        self.buf.write(uapi::as_maybe_uninit_bytes(s.as_bytes()));
        let none = [MaybeUninit::new(0); 4];
        self.buf.write(&none[..cap - len + 1]);
        self
    }

    pub fn fd(&mut self, fd: OwnedFd) -> &mut Self {
        self.fds.push(fd);
        self
    }

    pub fn object(&mut self, obj: ObjectId) -> &mut Self {
        self.uint(obj.raw())
    }

    pub fn header(&mut self, obj: ObjectId, event: u32) -> &mut Self {
        self.object(obj).uint(event)
    }
}

impl<'a> Drop for MsgFormatter<'a> {
    fn drop(&mut self) {
        assert!(self.buf.out_pos - self.pos >= 8);
        assert_eq!(self.pos % 4, 0);
        unsafe {
            let second_ptr = (self.buf.out_buf as *mut u8).add(self.pos + 4) as *mut u32;
            let len = ((self.buf.out_pos - self.pos) as u32) << 16;
            *second_ptr |= len;
        }
        if self.fds.len() > 0 {
            self.buf.fds.push_back(MsgFds {
                pos: self.pos,
                fds: mem::take(&mut self.fds),
            })
        }
    }
}
