use {
    crate::dbus::{DbusError, DbusType, Formatter, Message, MethodCall, Parser},
    std::{borrow::Cow, marker::PhantomData},
};

#[derive(Debug)]
pub struct Get<'a, T: DbusType<'static>> {
    pub interface_name: Cow<'a, str>,
    pub property_name: Cow<'a, str>,
    pub _phantom: PhantomData<T>,
}

unsafe impl<'a, T: DbusType<'static>> Message<'a> for Get<'a, T> {
    const SIGNATURE: &'static str = "ss";
    const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
    const MEMBER: &'static str = "Get";
    type Generic<'b> = Get<'b, T>;

    fn marshal(&self, fmt: &mut Formatter) {
        fmt.marshal(&self.interface_name);
        fmt.marshal(&self.property_name);
    }

    fn unmarshal(parser: &mut Parser<'a>) -> Result<Self, DbusError> {
        Ok(Self {
            interface_name: parser.unmarshal()?,
            property_name: parser.unmarshal()?,
            _phantom: Default::default(),
        })
    }

    fn num_fds(&self) -> u32 {
        0
    }
}

impl<'a, T: DbusType<'static>> MethodCall<'a> for Get<'a, T> {
    type Reply = GetReply<'static, T>;
}

#[derive(Debug)]
pub struct GetReply<'a, T: DbusType<'a>> {
    pub value: T,
    pub _phantom: PhantomData<&'a ()>,
}

unsafe impl<'a, T: DbusType<'a>> Message<'a> for GetReply<'a, T> {
    const SIGNATURE: &'static str = "v";
    const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
    const MEMBER: &'static str = "Get";
    type Generic<'b> = GetReply<'b, T::Generic<'b>>;

    fn marshal(&self, _fmt: &mut Formatter) {
        unimplemented!();
    }

    fn unmarshal(parser: &mut Parser<'a>) -> Result<Self, DbusError> {
        Ok(Self {
            value: parser.read_variant_as()?,
            _phantom: Default::default(),
        })
    }

    fn num_fds(&self) -> u32 {
        self.value.num_fds()
    }
}
