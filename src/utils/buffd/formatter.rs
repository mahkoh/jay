use {
    crate::{
        fixed::Fixed,
        object::ObjectId,
        utils::buffd::buf_out::{MsgFds, OUT_BUF_SIZE, OutBuffer, OutBufferMeta},
    },
    std::{mem, rc::Rc},
    uapi::{OwnedFd, Packed},
};

pub struct MsgFormatter<'a> {
    buf: &'a mut [u8],
    meta: &'a mut OutBufferMeta,
    pos: usize,
    fds: &'a mut Vec<Rc<OwnedFd>>,
}

impl<'a> MsgFormatter<'a> {
    pub fn new(buf: &'a mut OutBuffer, fds: &'a mut Vec<Rc<OwnedFd>>) -> Self {
        Self {
            pos: buf.meta.write_pos,
            buf: &mut buf.buf[..],
            fds,
            meta: &mut buf.meta,
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        if bytes.len() > OUT_BUF_SIZE - self.meta.write_pos {
            panic!("Out buffer overflow");
        }
        self.buf[self.meta.write_pos..self.meta.write_pos + bytes.len()].copy_from_slice(bytes);
        self.meta.write_pos += bytes.len();
    }

    pub fn int(&mut self, int: i32) -> &mut Self {
        self.write(uapi::as_bytes(&int));
        self
    }

    pub fn uint(&mut self, int: u32) -> &mut Self {
        self.write(uapi::as_bytes(&int));
        self
    }

    pub fn u64(&mut self, int: u64) -> &mut Self {
        self.uint((int >> 32) as u32);
        self.uint(int as u32)
    }

    pub fn u64_rev(&mut self, int: u64) -> &mut Self {
        self.uint(int as u32);
        self.uint((int >> 32) as u32)
    }

    pub fn fixed(&mut self, fixed: Fixed) -> &mut Self {
        self.write(uapi::as_bytes(&fixed.0));
        self
    }

    pub fn optstr<S: AsRef<[u8]> + ?Sized>(&mut self, s: Option<&S>) -> &mut Self {
        match s {
            Some(s) => self.string(s),
            _ => self.uint(0),
        }
    }

    pub fn string<S: AsRef<[u8]> + ?Sized>(&mut self, s: &S) -> &mut Self {
        let s = s.as_ref();
        let len = s.len() + 1;
        let cap = (len + 3) & !3;
        self.uint(len as u32);
        self.write(uapi::as_bytes(s));
        let none = [0; 4];
        self.write(&none[..cap - len + 1]);
        self
    }

    pub fn fd(&mut self, fd: Rc<OwnedFd>) -> &mut Self {
        self.fds.push(fd);
        self
    }

    pub fn object<T: Into<ObjectId>>(&mut self, obj: T) -> &mut Self {
        self.uint(obj.into().raw())
    }

    pub fn header<T: Into<ObjectId>>(&mut self, obj: T, event: u32) -> &mut Self {
        self.object(obj).uint(event)
    }

    #[expect(dead_code)]
    pub fn array<F: FnOnce(&mut MsgFormatter<'_>)>(&mut self, f: F) -> &mut Self {
        let pos = self.meta.write_pos;
        self.uint(0);
        let len = {
            let mut fmt = MsgFormatter {
                buf: self.buf,
                meta: self.meta,
                pos,
                fds: self.fds,
            };
            f(&mut fmt);
            let len = self.meta.write_pos - pos - 4;
            let none = [0; 4];
            self.write(&none[..self.meta.write_pos.wrapping_neg() & 3]);
            len as u32
        };
        self.buf[pos..pos + 4].copy_from_slice(uapi::as_bytes(&len));
        self
    }

    pub fn binary<T: ?Sized + Packed>(&mut self, t: &T) -> &mut Self {
        self.uint(size_of_val(t) as u32);
        self.write(uapi::as_bytes(t));
        let none = [0; 4];
        self.write(&none[..self.meta.write_pos.wrapping_neg() & 3]);
        self
    }

    pub fn write_len(self) {
        assert!(self.meta.write_pos - self.pos >= 8);
        assert_eq!(self.pos % 4, 0);
        unsafe {
            let second_ptr = self.buf.as_ptr().add(self.pos + 4) as *mut u32;
            let len = ((self.meta.write_pos - self.pos) as u32) << 16;
            *second_ptr |= len;
        }
        if self.fds.len() > 0 {
            self.meta.fds.push_back(MsgFds {
                pos: self.pos,
                fds: mem::take(self.fds),
            })
        }
    }
}
