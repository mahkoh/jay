use {
    crate::{
        it::{
            test_error::TestError, test_ifs::test_surface::TestSurface, test_object::TestObject,
            test_transport::TestTransport,
        },
        wire::{wl_compositor::CreateSurface, WlCompositorId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestCompositor {
    pub id: WlCompositorId,
    pub tran: Rc<TestTransport>,
}

impl TestCompositor {
    pub async fn create_surface(&self) -> Result<Rc<TestSurface>, TestError> {
        let id = self.tran.id();
        self.tran.send(CreateSurface {
            self_id: self.id,
            id,
        });
        self.tran.sync().await;
        let client = self.tran.get_client()?;
        let server = client.lookup(id)?;
        let surface = Rc::new(TestSurface {
            id,
            tran: self.tran.clone(),
            server,
            destroyed: Cell::new(false),
        });
        self.tran.add_obj(surface.clone())?;
        Ok(surface)
    }
}

test_object! {
    TestCompositor, WlCompositor;
}

impl TestObject for TestCompositor {}
