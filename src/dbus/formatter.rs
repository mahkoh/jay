use {
    crate::{
        dbus::{types::Variant, DbusType, Formatter},
        utils::buf::DynamicBuf,
    },
    std::rc::Rc,
    uapi::{OwnedFd, Packed},
};

impl<'a> Formatter<'a> {
    pub fn new(fds: &'a mut Vec<Rc<OwnedFd>>, buf: &'a mut DynamicBuf) -> Self {
        Self { fds, buf }
    }

    pub fn marshal<'b, T: DbusType<'b>>(&mut self, t: &T) {
        t.marshal(self)
    }

    pub fn pad_to(&mut self, alignment: usize) {
        static BUF: [u8; 8] = [0; 8];
        let len = self.buf.len().wrapping_neg() & (alignment - 1);
        self.buf.extend_from_slice(&BUF[..len]);
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn write_packed<'b, T: DbusType<'b> + Packed>(&mut self, t: &T) {
        self.pad_to(T::ALIGNMENT);
        self.buf.extend_from_slice(uapi::as_bytes(t));
    }

    pub fn write_str(&mut self, s: &str) {
        self.write_packed(&(s.len() as u32));
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }

    pub fn write_signature(&mut self, s: &[u8]) {
        self.write_packed(&(s.len() as u8));
        self.buf.extend_from_slice(s);
        self.buf.push(0);
    }

    pub fn write_array<'b, T: DbusType<'b>>(&mut self, a: &[T]) {
        self.pad_to(4);
        let len_pos = self.buf.len();
        self.write_packed(&0u32);
        self.pad_to(T::ALIGNMENT);
        let start = self.buf.len();
        for v in a {
            v.marshal(self);
        }
        let len = (self.buf.len() - start) as u32;
        self.buf[len_pos..len_pos + 4].copy_from_slice(uapi::as_bytes(&len));
    }

    pub fn write_fd(&mut self, fd: &Rc<OwnedFd>) {
        self.write_packed(&(self.fds.len() as u32));
        self.fds.push(fd.clone());
    }

    pub fn write_variant(&mut self, variant: &Variant) {
        let pos = self.buf.len();
        self.buf.push(0);
        variant.write_signature(self.buf);
        self.buf.push(0);
        self.buf[pos] = (self.buf.len() - pos - 2) as u8;
        self.write_variant_body(variant);
    }

    pub fn write_variant_body(&mut self, variant: &Variant) {
        match variant {
            Variant::U8(v) => v.marshal(self),
            Variant::Bool(v) => v.marshal(self),
            Variant::I16(v) => v.marshal(self),
            Variant::U16(v) => v.marshal(self),
            Variant::I32(v) => v.marshal(self),
            Variant::U32(v) => v.marshal(self),
            Variant::I64(v) => v.marshal(self),
            Variant::U64(v) => v.marshal(self),
            Variant::F64(v) => v.marshal(self),
            Variant::String(v) => v.marshal(self),
            Variant::ObjectPath(v) => v.marshal(self),
            Variant::Signature(v) => v.marshal(self),
            Variant::Variant(v) => v.marshal(self),
            Variant::Fd(f) => f.marshal(self),
            Variant::Array(el, v) => {
                self.pad_to(4);
                let len_pos = self.buf.len();
                self.write_packed(&0u32);
                self.pad_to(el.alignment());
                let start = self.buf.len();
                for v in v {
                    self.write_variant_body(v);
                }
                let len = (self.buf.len() - start) as u32;
                self.buf[len_pos..len_pos + 4].copy_from_slice(uapi::as_bytes(&len));
            }
            Variant::DictEntry(k, v) => {
                self.pad_to(8);
                self.write_variant_body(k);
                self.write_variant_body(v);
            }
            Variant::Struct(f) => {
                self.pad_to(8);
                for v in f {
                    self.write_variant_body(v);
                }
            }
        }
    }
}
