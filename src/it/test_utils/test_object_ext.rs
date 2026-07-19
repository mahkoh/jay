use crate::it::test_error::TestError;
use crate::object::Object;
use std::any::Any;
use std::rc::Rc;

pub trait TestObjectExt {
    fn downcast<T: 'static>(self: Rc<Self>) -> Result<Rc<T>, TestError>;
}

impl TestObjectExt for dyn Object {
    fn downcast<T: 'static>(self: Rc<Self>) -> Result<Rc<T>, TestError> {
        match (self as Rc<dyn Any>).downcast() {
            Ok(t) => Ok(t),
            _ => bail!("Object has an incompatible type id"),
        }
    }
}
