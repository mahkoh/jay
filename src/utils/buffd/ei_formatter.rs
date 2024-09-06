use {
    crate::{
        ei::ei_object::EiObjectId,
        utils::buffd::buf_out::{MsgFds, OutBuffer, OutBufferMeta, OUT_BUF_SIZE},
    },
    std::{mem, rc::Rc},
    uapi::OwnedFd,
};

pub struct EiMsgFormatter<'a> {
    buf: &'a mut [u8],
    meta: &'a mut OutBufferMeta,
    pos: usize,
    fds: &'a mut Vec<Rc<OwnedFd>>,
}

impl<'a> EiMsgFormatter<'a> {
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

    #[expect(dead_code)]
    pub fn long(&mut self, int: i64) -> &mut Self {
        self.write(uapi::as_bytes(&int));
        self
    }

    pub fn ulong(&mut self, int: u64) -> &mut Self {
        self.write(uapi::as_bytes(&int));
        self
    }

    pub fn float(&mut self, f: f32) -> &mut Self {
        self.write(uapi::as_bytes(&f));
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

    pub fn object<T: Into<EiObjectId>>(&mut self, obj: T) -> &mut Self {
        self.ulong(obj.into().raw())
    }

    pub fn header<T: Into<EiObjectId>>(&mut self, obj: T, event: u32) -> &mut Self {
        self.object(obj).uint(0).uint(event)
    }

    pub fn write_len(self) {
        assert!(self.meta.write_pos - self.pos >= 16);
        assert_eq!(self.pos % 4, 0);
        unsafe {
            let second_ptr = self.buf.as_ptr().add(self.pos + 8) as *mut u32;
            *second_ptr = (self.meta.write_pos - self.pos) as u32;
        }
        if self.fds.len() > 0 {
            self.meta.fds.push_back(MsgFds {
                pos: self.pos,
                fds: mem::take(self.fds),
            })
        }
    }
}
