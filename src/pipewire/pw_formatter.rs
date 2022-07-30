use {
    crate::pipewire::pw_pod::{
        PW_TYPE_Array, PW_TYPE_Bitmap, PW_TYPE_Bool, PW_TYPE_Bytes, PW_TYPE_Choice, PW_TYPE_Double,
        PW_TYPE_Fd, PW_TYPE_Float, PW_TYPE_Fraction, PW_TYPE_Id, PW_TYPE_Int, PW_TYPE_Long,
        PW_TYPE_None, PW_TYPE_Object, PW_TYPE_Rectangle, PW_TYPE_String, PW_TYPE_Struct,
        PwChoiceType, PwPodObjectType, PwPodType, PwPropFlag,
    },
    std::rc::Rc,
    uapi::OwnedFd,
};

pub struct PwFormatter<'a> {
    data: &'a mut Vec<u8>,
    fds: &'a mut Vec<Rc<OwnedFd>>,
    array: bool,
    first: bool,
}

impl<'a> PwFormatter<'a> {
    pub fn write_bool(&mut self, b: bool) {
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&4u32));
            self.data.extend_from_slice(uapi::as_bytes(&PW_TYPE_Bool.0));
        }
        self.data.extend_from_slice(uapi::as_bytes(&(b as u32)));
        if !self.array {
            self.data.extend_from_slice(uapi::as_bytes(&0u32));
        }
        self.first = false;
    }

    pub fn write_id(&mut self, id: u32) {
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&4u32));
            self.data.extend_from_slice(uapi::as_bytes(&PW_TYPE_Id.0));
        }
        self.data.extend_from_slice(uapi::as_bytes(&id));
        if !self.array {
            self.data.extend_from_slice(uapi::as_bytes(&0u32));
        }
        self.first = false;
    }

    pub fn write_object<F>(&mut self, ty: PwPodObjectType, id: u32, f: F)
    where
        F: FnOnce(&mut PwObjectFormatter),
    {
        let start = self.data.len();
        self.data.extend_from_slice(uapi::as_bytes(&0u32));
        self.data
            .extend_from_slice(uapi::as_bytes(&PW_TYPE_Object.0));
        self.data.extend_from_slice(uapi::as_bytes(&ty.0));
        self.data.extend_from_slice(uapi::as_bytes(&id));
        let mut fmt = PwObjectFormatter {
            data: self.data,
            fds: self.fds,
        };
        f(&mut fmt);
        let len = (self.data.len() - start - 8) as u32;
        self.data[start..start + 4].copy_from_slice(uapi::as_bytes(&len));
    }

    pub fn write_uint(&mut self, int: u32) {
        self.write_int(int as _)
    }

    pub fn write_int(&mut self, int: i32) {
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&4u32));
            self.data.extend_from_slice(uapi::as_bytes(&PW_TYPE_Int.0));
        }
        self.data.extend_from_slice(uapi::as_bytes(&int));
        if !self.array {
            self.data.extend_from_slice(uapi::as_bytes(&0u32));
        }
        self.first = false;
    }

    pub fn write_ulong(&mut self, long: u64) {
        self.write_long(long as _)
    }

    pub fn write_long(&mut self, long: i64) {
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&8u32));
            self.data.extend_from_slice(uapi::as_bytes(&PW_TYPE_Long.0));
        }
        self.data.extend_from_slice(uapi::as_bytes(&long));
        self.first = false;
    }

    #[allow(dead_code)]
    pub fn write_float(&mut self, float: f32) {
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&4u32));
            self.data
                .extend_from_slice(uapi::as_bytes(&PW_TYPE_Float.0));
        }
        self.data.extend_from_slice(uapi::as_bytes(&float));
        if !self.array {
            self.data.extend_from_slice(uapi::as_bytes(&0u32));
        }
        self.first = false;
    }

    #[allow(dead_code)]
    pub fn write_double(&mut self, double: f64) {
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&8u32));
            self.data
                .extend_from_slice(uapi::as_bytes(&PW_TYPE_Double.0));
        }
        self.data.extend_from_slice(uapi::as_bytes(&double));
    }

    pub fn write_string<S: AsRef<[u8]> + ?Sized>(&mut self, s: &S) {
        let s = s.as_ref();
        self.data
            .extend_from_slice(uapi::as_bytes(&(s.len() as u32 + 1)));
        self.data
            .extend_from_slice(uapi::as_bytes(&PW_TYPE_String.0));
        self.data.extend_from_slice(s);
        self.data.push(0);
        self.pad();
    }

    #[allow(dead_code)]
    pub fn write_bytes(&mut self, s: &[u8]) {
        self.data
            .extend_from_slice(uapi::as_bytes(&(s.len() as u32)));
        self.data
            .extend_from_slice(uapi::as_bytes(&PW_TYPE_Bytes.0));
        self.data.extend_from_slice(s);
        self.pad();
    }

    pub fn write_rectangle(&mut self, width: u32, height: u32) {
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&8u32));
            self.data
                .extend_from_slice(uapi::as_bytes(&PW_TYPE_Rectangle.0));
        }
        self.data.extend_from_slice(uapi::as_bytes(&width));
        self.data.extend_from_slice(uapi::as_bytes(&height));
        self.first = false;
    }

    #[allow(dead_code)]
    pub fn write_fraction(&mut self, num: i32, denom: i32) {
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&8u32));
            self.data
                .extend_from_slice(uapi::as_bytes(&PW_TYPE_Fraction.0));
        }
        self.data.extend_from_slice(uapi::as_bytes(&num));
        self.data.extend_from_slice(uapi::as_bytes(&denom));
        self.first = false;
    }

    pub fn write_none(&mut self) {
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&0u32));
            self.data.extend_from_slice(uapi::as_bytes(&PW_TYPE_None.0));
        }
        self.first = false;
    }

    #[allow(dead_code)]
    pub fn write_bitmap(&mut self, s: &[u8]) {
        self.data
            .extend_from_slice(uapi::as_bytes(&(s.len() as u32)));
        self.data
            .extend_from_slice(uapi::as_bytes(&PW_TYPE_Bitmap.0));
        self.data.extend_from_slice(s);
        self.pad();
    }

    #[allow(dead_code)]
    pub fn write_fd(&mut self, fd: &Rc<OwnedFd>) {
        let pos = self.fds.len() as u64;
        self.fds.push(fd.clone());
        if !self.array || self.first {
            self.data.extend_from_slice(uapi::as_bytes(&8u32));
            self.data.extend_from_slice(uapi::as_bytes(&PW_TYPE_Fd.0));
        }
        self.data.extend_from_slice(uapi::as_bytes(&pos));
        self.first = false;
    }

    pub fn write_struct<F>(&mut self, f: F)
    where
        F: FnOnce(&mut PwFormatter),
    {
        self.write_compound(PW_TYPE_Struct, |fmt| {
            let mut fmt = PwFormatter {
                data: fmt.data,
                fds: fmt.fds,
                array: false,
                first: false,
            };
            f(&mut fmt);
        });
    }

    #[allow(dead_code)]
    pub fn write_array<F>(&mut self, f: F)
    where
        F: FnOnce(&mut PwFormatter),
    {
        self.write_compound(PW_TYPE_Array, |fmt| {
            fmt.write_array_body(f);
        });
        self.pad();
    }

    fn write_array_body<F>(&mut self, f: F)
    where
        F: FnOnce(&mut PwFormatter),
    {
        let mut fmt = PwFormatter {
            data: self.data,
            fds: self.fds,
            array: true,
            first: true,
        };
        f(&mut fmt);
        if fmt.first {
            fmt.write_none();
        }
    }

    pub fn write_choice<F>(&mut self, ty: PwChoiceType, flags: u32, f: F)
    where
        F: FnOnce(&mut PwFormatter),
    {
        self.write_compound(PW_TYPE_Choice, |fmt| {
            fmt.data.extend_from_slice(uapi::as_bytes(&ty.0));
            fmt.data.extend_from_slice(uapi::as_bytes(&flags));
            fmt.write_array_body(f);
        });
        self.pad();
    }

    fn write_compound<F>(&mut self, ty: PwPodType, f: F)
    where
        F: FnOnce(&mut PwFormatter),
    {
        let start = self.data.len();
        self.data.extend_from_slice(uapi::as_bytes(&0u32));
        self.data.extend_from_slice(uapi::as_bytes(&ty.0));
        f(self);
        let len = (self.data.len() - start - 8) as u32;
        self.data[start..start + 4].copy_from_slice(uapi::as_bytes(&len));
    }

    fn pad(&mut self) {
        let todo = self.data.len().wrapping_neg() & 7;
        self.data.extend_from_slice(&uapi::as_bytes(&0u64)[..todo]);
    }
}

pub struct PwObjectFormatter<'a> {
    data: &'a mut Vec<u8>,
    fds: &'a mut Vec<Rc<OwnedFd>>,
}

impl<'a> PwObjectFormatter<'a> {
    pub fn write_property<F>(&mut self, key: u32, flags: PwPropFlag, f: F)
    where
        F: FnOnce(&mut PwFormatter),
    {
        self.data.extend_from_slice(uapi::as_bytes(&key));
        self.data.extend_from_slice(uapi::as_bytes(&flags.0));
        let mut fmt = PwFormatter {
            data: self.data,
            fds: self.fds,
            array: false,
            first: false,
        };
        f(&mut fmt);
    }
}

pub fn format<F>(buf: &mut Vec<u8>, fds: &mut Vec<Rc<OwnedFd>>, id: u32, opcode: u8, seq: u32, f: F)
where
    F: FnOnce(&mut PwFormatter),
{
    buf.clear();
    buf.extend_from_slice(uapi::as_bytes(&id));
    buf.extend_from_slice(uapi::as_bytes(&0u32));
    buf.extend_from_slice(uapi::as_bytes(&seq));
    buf.extend_from_slice(uapi::as_bytes(&0u32));
    let mut fmt = PwFormatter {
        data: buf,
        fds,
        array: false,
        first: false,
    };
    f(&mut fmt);
    let p2 = (buf.len() - 16) as u32 | ((opcode as u32) << 24);
    buf[4..8].copy_from_slice(uapi::as_bytes(&p2));
    let nfds = fds.len() as u32;
    buf[12..16].copy_from_slice(uapi::as_bytes(&nfds));
}
