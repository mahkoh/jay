use {
    crate::xcon::Message,
    std::rc::Rc,
    uapi::{AssertPacked, OwnedFd, Packed},
};

pub struct Formatter<'a> {
    fds: &'a mut Vec<Rc<OwnedFd>>,
    buf: &'a mut Vec<u8>,
    ext_opcode: u8,
}

impl<'a> Formatter<'a> {
    pub fn new(fds: &'a mut Vec<Rc<OwnedFd>>, buf: &'a mut Vec<u8>, ext_opcode: u8) -> Self {
        Self {
            fds,
            buf,
            ext_opcode,
        }
    }

    pub fn ext_opcode(&self) -> u8 {
        self.ext_opcode
    }

    pub fn pad(&mut self, pad: usize) {
        static BUF: [u8; 8] = [0; 8];
        self.buf.extend_from_slice(&BUF[..pad]);
    }

    pub fn pad_to(&mut self, size: usize) {
        static BUF: [u8; 8] = [0; 8];
        while self.buf.len() < size {
            let len = (size - self.buf.len()).min(8);
            self.buf.extend_from_slice(&BUF[..len]);
        }
    }

    pub fn align(&mut self, alignment: usize) {
        static BUF: [u8; 8] = [0; 8];
        let len = self.buf.len().wrapping_neg() & (alignment - 1);
        self.buf.extend_from_slice(&BUF[..len]);
    }

    pub fn write_packed<T: Packed + ?Sized>(&mut self, t: &T) {
        self.buf.extend_from_slice(uapi::as_bytes(t));
    }

    pub fn write_list<'b, T: Message<'b>>(&mut self, t: &[T]) {
        if T::IS_POD {
            self.buf
                .extend_from_slice(uapi::as_bytes(unsafe { AssertPacked::new(t) }));
        } else {
            for t in t {
                t.serialize(self);
            }
        }
    }

    pub fn write_bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }

    pub fn write_request_length(&mut self) {
        let len: u16 = (self.buf.len() / 4) as u16;
        self.buf[2..4].copy_from_slice(&len.to_ne_bytes());
    }

    pub fn add_fd(&mut self, fd: &Rc<OwnedFd>) {
        self.fds.push(fd.clone());
    }
}
