use {
    crate::{
        it::{test_object::TestObject, test_transport::TestTransport},
        wire::WlCompositorId,
    },
    std::rc::Rc,
};

pub struct TestCompositor {
    pub id: WlCompositorId,
    pub transport: Rc<TestTransport>,
}

test_object! {
    TestCompositor, WlCompositor;
}

impl TestObject for TestCompositor {}
