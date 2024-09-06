use {
    crate::{
        it::{test_error::TestError, test_transport::TestTransport},
        object::{Interface, ObjectId},
        utils::buffd::MsgParser,
    },
    std::rc::Rc,
};

macro_rules! test_object {
    ($oname:ident, $ifname:ident; $($code:ident => $f:ident,)*) => {
        impl crate::it::test_object::TestObjectBase for $oname {
            fn id(&self) -> crate::object::ObjectId {
                self.id.into()
            }

            #[allow(clippy::allow_attributes, unused_variables, unreachable_code)]
            fn handle_request(
                self: std::rc::Rc<Self>,
                request: u32,
                parser: crate::utils::buffd::MsgParser<'_, '_>,
            ) -> Result<(), crate::it::test_error::TestError> {
                use crate::it::test_error::TestErrorExt;
                let res: Result<(), crate::it::test_error::TestError> = match request {
                    $(
                        $code => $oname::$f(&self, parser).with_context(|| format!("While handling a `{}` event", stringify!($f))),
                    )*
                    _ => Err(crate::it::test_error::TestError::new(format!("Unknown event {}", request))),
                };
                res.with_context(|| format!("In object {} of type `{}`", self.id(), self.interface().name()))
            }

            fn interface(&self) -> crate::object::Interface {
                crate::wire::$ifname
            }
        }
    };
}

pub trait TestObjectBase: 'static {
    fn id(&self) -> ObjectId;
    fn handle_request(
        self: Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), TestError>;
    fn interface(&self) -> Interface;
}

pub trait TestObject: TestObjectBase {
    fn on_remove(&self, transport: &TestTransport) {
        let _ = transport;
    }
}
