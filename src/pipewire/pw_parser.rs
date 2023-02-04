#![allow(non_upper_case_globals)]

use {
    crate::pipewire::pw_pod::{
        PW_CHOICE_None, PW_TYPE_Array, PW_TYPE_Bitmap, PW_TYPE_Bool, PW_TYPE_Bytes, PW_TYPE_Choice,
        PW_TYPE_Double, PW_TYPE_Fd, PW_TYPE_Float, PW_TYPE_Fraction, PW_TYPE_Id, PW_TYPE_Int,
        PW_TYPE_Long, PW_TYPE_None, PW_TYPE_Object, PW_TYPE_Pod, PW_TYPE_Pointer,
        PW_TYPE_Rectangle, PW_TYPE_Sequence, PW_TYPE_String, PW_TYPE_Struct, PwChoiceType,
        PwControlType, PwPod, PwPodArray, PwPodChoice, PwPodControl, PwPodFraction, PwPodObject,
        PwPodObjectType, PwPodPointer, PwPodRectangle, PwPodSequence, PwPodStruct, PwPodType,
        PwPointerType, PwProp, PwPropFlag,
    },
    ahash::AHashMap,
    bstr::{BStr, BString, ByteSlice},
    std::{
        fmt::Debug,
        mem::{self, MaybeUninit},
        rc::Rc,
    },
    thiserror::Error,
    uapi::{OwnedFd, Pod},
};

#[derive(Debug, Error)]
pub enum PwParserError {
    #[error("Unexpected EOF")]
    UnexpectedEof,
    #[error("Message references an FD that is out of bounds")]
    MissingFd,
    #[error("Array element type has size of 0")]
    ZeroSizedArrayElementType,
    #[error("Unknown POD type: {0:?}")]
    UnknownType(PwPodType),
    #[error("Unexpected POD type: Expected {0:?}, got {1:?}")]
    UnexpectedPodType(PwPodType, PwPodType),
}

#[derive(Copy, Clone)]
pub struct PwParser<'a> {
    data: &'a [u8],
    fds: &'a [Rc<OwnedFd>],
    pos: usize,
}

impl<'a> PwParser<'a> {
    pub fn new(data: &'a [u8], fds: &'a [Rc<OwnedFd>]) -> Self {
        Self { data, fds, pos: 0 }
    }

    pub fn reset(&mut self) {
        self.pos = 0;
    }

    fn read_raw<T: Pod>(&mut self, offset: usize) -> Result<T, PwParserError> {
        if self.pos + offset + mem::size_of::<T>() <= self.data.len() {
            unsafe {
                let mut res = MaybeUninit::uninit();
                let src = self.data[self.pos + offset..].as_ptr();
                std::ptr::copy_nonoverlapping(src, res.as_mut_ptr() as _, mem::size_of::<T>());
                Ok(res.assume_init())
            }
        } else {
            Err(PwParserError::UnexpectedEof)
        }
    }

    pub fn len(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    fn read_array(&mut self, offset: usize, len: usize) -> Result<PwPodArray<'a>, PwParserError> {
        let child_len = self.read_raw::<u32>(offset)? as usize;
        if child_len == 0 {
            return Err(PwParserError::ZeroSizedArrayElementType);
        }
        let ty = PwPodType(self.read_raw(offset + 4)?);
        Ok(PwPodArray {
            ty,
            child_len,
            n_elements: (len - 8) / child_len,
            elements: PwParser::new(
                &self.data[self.pos + offset + 8..self.pos + offset + len],
                self.fds,
            ),
        })
    }

    pub fn read_dict_struct(&mut self) -> Result<AHashMap<BString, BString>, PwParserError> {
        let s2 = self.read_struct()?;
        let mut p3 = s2.fields;
        let num_dict_entries = p3.read_int()?;
        let mut de = AHashMap::new();
        for _ in 0..num_dict_entries {
            de.insert(p3.read_string()?.to_owned(), p3.read_string()?.to_owned());
        }
        Ok(de)
    }

    pub fn read_struct(&mut self) -> Result<PwPodStruct<'a>, PwParserError> {
        match self.read_pod()? {
            PwPod::Struct(s) => Ok(s),
            v => Err(PwParserError::UnexpectedPodType(PW_TYPE_Struct, v.ty())),
        }
    }

    pub fn read_uint(&mut self) -> Result<u32, PwParserError> {
        self.read_int().map(|v| v as u32)
    }

    pub fn read_int(&mut self) -> Result<i32, PwParserError> {
        match self.read_value()? {
            PwPod::Int(s) => Ok(s),
            v => Err(PwParserError::UnexpectedPodType(PW_TYPE_Int, v.ty())),
        }
    }

    pub fn read_object_opt(&mut self) -> Result<Option<PwPodObject>, PwParserError> {
        match self.read_pod()? {
            PwPod::Object(p) => Ok(Some(p)),
            PwPod::None => Ok(None),
            v => Err(PwParserError::UnexpectedPodType(PW_TYPE_Object, v.ty())),
        }
    }

    pub fn read_object(&mut self) -> Result<PwPodObject, PwParserError> {
        match self.read_object_opt()? {
            Some(p) => Ok(p),
            _ => Err(PwParserError::UnexpectedPodType(
                PW_TYPE_Object,
                PW_TYPE_None,
            )),
        }
    }

    pub fn read_id(&mut self) -> Result<u32, PwParserError> {
        match self.read_value()? {
            PwPod::Id(s) => Ok(s),
            v => Err(PwParserError::UnexpectedPodType(PW_TYPE_Id, v.ty())),
        }
    }

    pub fn read_fd_opt(&mut self) -> Result<Option<Rc<OwnedFd>>, PwParserError> {
        match self.read_pod()? {
            PwPod::Fd(idx) if idx == !0 => Ok(None),
            PwPod::Fd(idx) => match self.fds.get(idx as usize) {
                Some(fd) => Ok(Some(fd.clone())),
                _ => Err(PwParserError::MissingFd),
            },
            v => Err(PwParserError::UnexpectedPodType(PW_TYPE_Id, v.ty())),
        }
    }

    pub fn read_fd(&mut self) -> Result<Rc<OwnedFd>, PwParserError> {
        match self.read_fd_opt()? {
            Some(fd) => Ok(fd),
            _ => Err(PwParserError::MissingFd),
        }
    }

    pub fn read_ulong(&mut self) -> Result<u64, PwParserError> {
        self.read_long().map(|l| l as _)
    }

    pub fn read_long(&mut self) -> Result<i64, PwParserError> {
        match self.read_value()? {
            PwPod::Long(s) => Ok(s),
            v => Err(PwParserError::UnexpectedPodType(PW_TYPE_Long, v.ty())),
        }
    }

    pub fn read_string(&mut self) -> Result<&'a BStr, PwParserError> {
        match self.read_value()? {
            PwPod::String(s) => Ok(s),
            v => Err(PwParserError::UnexpectedPodType(PW_TYPE_String, v.ty())),
        }
    }

    pub fn read_value(&mut self) -> Result<PwPod<'a>, PwParserError> {
        let mut v = self.read_pod();
        if let Ok(PwPod::Choice(v)) = &mut v {
            if v.ty == PW_CHOICE_None && v.elements.n_elements > 0 {
                return v
                    .elements
                    .elements
                    .read_pod_body_packed(v.elements.ty, v.elements.child_len);
            }
        }
        v
    }

    pub fn read_pod(&mut self) -> Result<PwPod<'a>, PwParserError> {
        let len = self.read_raw::<u32>(0)? as usize;
        let ty = PwPodType(self.read_raw(4)?);
        self.pos += 8;
        self.read_pod_body(ty, len)
    }

    pub fn read_pod_body_packed(
        &mut self,
        ty: PwPodType,
        len: usize,
    ) -> Result<PwPod<'a>, PwParserError> {
        self.read_pod_body2(ty, len, true)
    }

    pub fn read_pod_body(&mut self, ty: PwPodType, len: usize) -> Result<PwPod<'a>, PwParserError> {
        self.read_pod_body2(ty, len, false)
    }

    fn read_pod_body2(
        &mut self,
        ty: PwPodType,
        len: usize,
        packed: bool,
    ) -> Result<PwPod<'a>, PwParserError> {
        let size = if packed { len } else { (len + 7) & !7 };
        if self.len() < size {
            return Err(PwParserError::UnexpectedEof);
        }
        let val = match ty {
            PW_TYPE_None => PwPod::None,
            PW_TYPE_Bool => PwPod::Bool(self.read_raw::<i32>(0)? != 0),
            PW_TYPE_Id => PwPod::Id(self.read_raw(0)?),
            PW_TYPE_Int => PwPod::Int(self.read_raw(0)?),
            PW_TYPE_Long => PwPod::Long(self.read_raw(0)?),
            PW_TYPE_Float => PwPod::Float(self.read_raw(0)?),
            PW_TYPE_Double => PwPod::Double(self.read_raw(0)?),
            PW_TYPE_String => {
                let s = if len == 0 {
                    &[][..]
                } else {
                    &self.data[self.pos..self.pos + len - 1]
                };
                PwPod::String(s.as_bstr())
            }
            PW_TYPE_Bytes => PwPod::Bytes(&self.data[self.pos..self.pos + len]),
            PW_TYPE_Rectangle => PwPod::Rectangle(PwPodRectangle {
                width: self.read_raw(0)?,
                height: self.read_raw(4)?,
            }),
            PW_TYPE_Fraction => PwPod::Fraction(PwPodFraction {
                num: self.read_raw(0)?,
                denom: self.read_raw(4)?,
            }),
            PW_TYPE_Bitmap => PwPod::Bitmap(&self.data[self.pos..self.pos + len]),
            PW_TYPE_Array => PwPod::Array(self.read_array(0, len)?),
            PW_TYPE_Struct => PwPod::Struct(PwPodStruct {
                fields: PwParser::new(&self.data[self.pos..self.pos + len], self.fds),
            }),
            PW_TYPE_Object => PwPod::Object(PwPodObject {
                ty: PwPodObjectType(self.read_raw(0)?),
                id: self.read_raw(4)?,
                probs: PwParser::new(&self.data[self.pos + 8..self.pos + len], self.fds),
            }),
            PW_TYPE_Sequence => PwPod::Sequence(PwPodSequence {
                unit: self.read_raw(0)?,
                controls: PwParser::new(&self.data[self.pos + 8..self.pos + len], self.fds),
            }),
            PW_TYPE_Pointer => PwPod::Pointer(PwPodPointer {
                ty: PwPointerType(self.read_raw(0)?),
                value: self.read_raw(8)?,
            }),
            PW_TYPE_Fd => PwPod::Fd(self.read_raw(0)?),
            PW_TYPE_Choice => PwPod::Choice(PwPodChoice {
                ty: PwChoiceType(self.read_raw(0)?),
                flags: self.read_raw(4)?,
                elements: self.read_array(8, len - 8)?,
            }),
            PW_TYPE_Pod => {
                let pos = self.pos;
                let pod = self.read_pod()?;
                self.pos = pos;
                pod
            }
            _ => return Err(PwParserError::UnknownType(ty)),
        };
        self.pos += size;
        Ok(val)
    }

    pub fn read_prop(&mut self) -> Result<PwProp<'a>, PwParserError> {
        let key = self.read_raw(0)?;
        let flags = PwPropFlag(self.read_raw(4)?);
        self.pos += 8;
        Ok(PwProp {
            key,
            flags,
            pod: self.read_pod()?,
        })
    }

    pub fn read_control(&mut self) -> Result<PwPodControl<'a>, PwParserError> {
        let offset = self.read_raw(0)?;
        let ty = PwControlType(self.read_raw(4)?);
        self.pos += 8;
        Ok(PwPodControl {
            offset,
            ty,
            value: self.read_pod()?,
        })
    }

    pub fn skip(&mut self) -> Result<(), PwParserError> {
        let size = self.read_raw::<u32>(0)? as usize;
        if self.len() < size + 8 {
            return Err(PwParserError::UnexpectedEof);
        }
        self.pos += size + 8;
        Ok(())
    }
}
