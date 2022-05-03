use {
    crate::{
        it::{
            test_error::TestError,
            test_ifs::{test_region::TestRegion, test_surface::TestSurface},
            test_object::{Deleted, TestObject},
            test_transport::TestTransport,
        },
        wire::{
            wl_compositor::{CreateRegion, CreateSurface},
            WlCompositorId,
        },
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestCompositor {
    pub id: WlCompositorId,
    pub tran: Rc<TestTransport>,
    pub deleted: Deleted,
}

impl TestCompositor {
    pub async fn create_surface(&self) -> Result<Rc<TestSurface>, TestError> {
        let id = self.tran.id();
        self.deleted.check()?;
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
            deleted: Default::default(),
        });
        self.tran.add_obj(surface.clone())?;
        Ok(surface)
    }

    pub async fn create_region(&self) -> Result<Rc<TestRegion>, TestError> {
        let id = self.tran.id();
        self.deleted.check()?;
        self.tran.send(CreateRegion {
            self_id: self.id,
            id,
        });
        self.tran.sync().await;
        let client = self.tran.get_client()?;
        let server = client.lookup(id)?;
        let region = Rc::new(TestRegion {
            id,
            tran: self.tran.clone(),
            server,
            destroyed: Cell::new(false),
            deleted: Default::default(),
            expected: Default::default(),
        });
        self.tran.add_obj(region.clone())?;
        Ok(region)
    }
}

test_object! {
    TestCompositor, WlCompositor;
}

impl TestObject for TestCompositor {}
