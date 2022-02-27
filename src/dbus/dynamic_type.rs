use super::{
    TY_ARRAY, TY_BOOLEAN, TY_BYTE, TY_DOUBLE, TY_INT16, TY_INT32, TY_INT64, TY_OBJECT_PATH,
    TY_SIGNATURE, TY_STRING, TY_UINT16, TY_UINT32, TY_UINT64, TY_UNIX_FD, TY_VARIANT,
};
use crate::dbus::types::Variant;
use crate::dbus::{DbusError, DynamicType, Parser};
use std::ops::Deref;

impl DynamicType {
    pub fn from_signature<'a>(mut s: &'a [u8]) -> Result<(DynamicType, &'a [u8]), DbusError> {
        if s.is_empty() {
            return Err(DbusError::EmptySignature);
        }
        let first = s[0];
        s = &s[1..];
        let dp = match first {
            TY_BYTE => DynamicType::U8,
            TY_BOOLEAN => DynamicType::Bool,
            TY_INT16 => DynamicType::I16,
            TY_UINT16 => DynamicType::U16,
            TY_INT32 => DynamicType::I32,
            TY_UINT32 => DynamicType::U32,
            TY_INT64 => DynamicType::I64,
            TY_UINT64 => DynamicType::U64,
            TY_DOUBLE => DynamicType::F64,
            TY_STRING => DynamicType::String,
            TY_OBJECT_PATH => DynamicType::ObjectPath,
            TY_SIGNATURE => DynamicType::Signature,
            TY_VARIANT => DynamicType::Variant,
            TY_UNIX_FD => DynamicType::Fd,
            TY_ARRAY => {
                let (elty, rem) = Self::from_signature(s)?;
                s = rem;
                DynamicType::Array(Box::new(elty))
            }
            b'{' => {
                let (keyty, rem) = Self::from_signature(s)?;
                let (valty, rem) = Self::from_signature(rem)?;
                if rem.is_empty() {
                    return Err(DbusError::UnterminatedDict);
                }
                if rem[0] != b'}' {
                    return Err(DbusError::DictTrailing);
                }
                s = &rem[1..];
                DynamicType::DictEntry(Box::new(keyty), Box::new(valty))
            }
            b'(' => {
                let mut fields = vec![];
                loop {
                    if s.is_empty() {
                        return Err(DbusError::UnterminatedStruct);
                    }
                    if s[0] == b')' {
                        s = &s[1..];
                        break DynamicType::Struct(fields);
                    }
                    let (fieldty, rem) = Self::from_signature(s)?;
                    s = rem;
                    fields.push(fieldty);
                }
            }
            _ => return Err(DbusError::UnknownType),
        };
        Ok((dp, s))
    }

    pub fn alignment(&self) -> usize {
        match self {
            DynamicType::U8 => 1,
            DynamicType::Bool => 4,
            DynamicType::I16 => 2,
            DynamicType::U16 => 2,
            DynamicType::I32 => 4,
            DynamicType::U32 => 4,
            DynamicType::I64 => 8,
            DynamicType::U64 => 8,
            DynamicType::F64 => 8,
            DynamicType::String => 4,
            DynamicType::ObjectPath => 4,
            DynamicType::Signature => 1,
            DynamicType::Variant => 1,
            DynamicType::Array(_) => 4,
            DynamicType::DictEntry(_, _) => 8,
            DynamicType::Struct(_) => 8,
            DynamicType::Fd => 4,
        }
    }

    pub fn write_signature(&self, w: &mut Vec<u8>) {
        let c = match self {
            DynamicType::U8 => TY_BYTE,
            DynamicType::Bool => TY_BOOLEAN,
            DynamicType::I16 => TY_INT16,
            DynamicType::U16 => TY_UINT16,
            DynamicType::I32 => TY_INT32,
            DynamicType::U32 => TY_UINT32,
            DynamicType::I64 => TY_INT64,
            DynamicType::U64 => TY_UINT64,
            DynamicType::F64 => TY_DOUBLE,
            DynamicType::String => TY_STRING,
            DynamicType::ObjectPath => TY_OBJECT_PATH,
            DynamicType::Signature => TY_SIGNATURE,
            DynamicType::Variant => TY_VARIANT,
            DynamicType::Fd => TY_UNIX_FD,
            DynamicType::Array(el) => {
                w.push(TY_ARRAY);
                el.write_signature(w);
                return;
            }
            DynamicType::DictEntry(k, v) => {
                w.push(b'{');
                k.write_signature(w);
                v.write_signature(w);
                w.push(b'}');
                return;
            }
            DynamicType::Struct(f) => {
                w.push(b'(');
                for f in f {
                    f.write_signature(w);
                }
                w.push(b')');
                return;
            }
        };
        w.push(c);
    }

    pub fn parse<'a>(&self, parser: &mut Parser<'a>) -> Result<Variant<'a>, DbusError> {
        let var = match self {
            DynamicType::U8 => Variant::U8(parser.read_pod()?),
            DynamicType::Bool => Variant::Bool(parser.read_bool()?),
            DynamicType::I16 => Variant::I16(parser.read_pod()?),
            DynamicType::U16 => Variant::U16(parser.read_pod()?),
            DynamicType::I32 => Variant::I32(parser.read_pod()?),
            DynamicType::U32 => Variant::U32(parser.read_pod()?),
            DynamicType::I64 => Variant::I64(parser.read_pod()?),
            DynamicType::U64 => Variant::U64(parser.read_pod()?),
            DynamicType::F64 => Variant::F64(parser.read_pod()?),
            DynamicType::String => Variant::String(parser.read_string()?),
            DynamicType::ObjectPath => Variant::ObjectPath(parser.read_object_path()?),
            DynamicType::Signature => Variant::Signature(parser.read_signature()?),
            DynamicType::Variant => Variant::Variant(Box::new(parser.read_variant()?)),
            DynamicType::Fd => Variant::Fd(parser.read_fd()?),
            DynamicType::Array(el) => {
                let len: u32 = parser.read_pod()?;
                parser.align_to(el.alignment());
                let len = len as usize;
                if parser.buf.len() - parser.pos < len {
                    return Err(DbusError::UnexpectedEof);
                }
                let mut vals = vec![];
                {
                    let mut parser = Parser {
                        buf: &parser.buf[..parser.pos + len],
                        pos: parser.pos,
                        fds: parser.fds,
                    };
                    while !parser.eof() {
                        vals.push(el.parse(&mut parser)?);
                    }
                }
                parser.pos += len;
                Variant::Array(el.deref().clone(), vals)
            }
            DynamicType::DictEntry(k, v) => {
                parser.align_to(8);
                Variant::DictEntry(Box::new(k.parse(parser)?), Box::new(v.parse(parser)?))
            }
            DynamicType::Struct(fields) => {
                let mut vals = vec![];
                parser.align_to(8);
                for field in fields {
                    vals.push(field.parse(parser)?);
                }
                Variant::Struct(vals)
            }
        };
        Ok(var)
    }
}
