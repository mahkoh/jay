use crate::object::ObjectId;
use crate::utils::buffd::buf_out::{BufFdOut, MsgFds};
use std::mem;
use std::mem::MaybeUninit;
use uapi::OwnedFd;
use crate::fixed::Fixed;

pub struct MsgFormatter<'a> {
    buf: &'a mut BufFdOut,
    pos: usize,
    fds: &'a mut Vec<OwnedFd>,
}

impl<'a> MsgFormatter<'a> {
    pub fn new(buf: &'a mut BufFdOut, fds: &'a mut Vec<OwnedFd>) -> Self {
        Self {
            pos: buf.out_pos,
            buf,
            fds,
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

    pub fn fixed(&mut self, fixed: Fixed) -> &mut Self {
        self.buf.write(uapi::as_maybe_uninit_bytes(&fixed));
        self
    }

    pub fn string<S: AsRef<[u8]> + ?Sized>(&mut self, s: &S) -> &mut Self {
        let s = s.as_ref();
        let len = s.len() + 1;
        let cap = (len + 3) & !3;
        self.uint(len as u32);
        self.buf.write(uapi::as_maybe_uninit_bytes(s));
        let none = [MaybeUninit::new(0); 4];
        self.buf.write(&none[..cap - len + 1]);
        self
    }

    pub fn fd(&mut self, fd: OwnedFd) -> &mut Self {
        self.fds.push(fd);
        self
    }

    pub fn object<T: Into<ObjectId>>(&mut self, obj: T) -> &mut Self {
        self.uint(obj.into().raw())
    }

    pub fn header<T: Into<ObjectId>>(&mut self, obj: T, event: u32) -> &mut Self {
        self.object(obj).uint(event)
    }

    pub fn array<F: FnOnce(&mut MsgFormatter<'_>)>(&mut self, f: F) -> &mut Self {
        let pos = self.buf.out_pos;
        self.uint(0);
        let len = {
            let mut fmt = MsgFormatter {
                buf: self.buf,
                pos,
                fds: self.fds,
            };
            f(&mut fmt);
            let len = self.buf.out_pos - pos + 4;
            let none = [MaybeUninit::new(0); 4];
            self.buf.write(&none[..self.buf.out_pos.wrapping_neg() & 3]);
            len as u32
        };
        unsafe {
            (*self.buf.out_buf)[pos..pos + 4].copy_from_slice(uapi::as_maybe_uninit_bytes(&len));
        }
        self
    }

    pub fn write_len(self) {
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
                fds: mem::take(self.fds),
            })
        }
    }
}
