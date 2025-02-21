use {
    crate::{
        it::{
            test_error::TestError, test_ifs::test_subsurface::TestSubsurface,
            test_object::TestObject, test_transport::TestTransport,
        },
        wire::{WlSubcompositorId, WlSurfaceId, wl_subcompositor::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestSubcompositor {
    pub id: WlSubcompositorId,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestSubcompositor {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            destroyed: Cell::new(false),
        }
    }

    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub async fn get_subsurface(
        &self,
        surface: WlSurfaceId,
        parent: WlSurfaceId,
    ) -> Result<Rc<TestSubsurface>, TestError> {
        let id = self.tran.id();
        self.tran.send(GetSubsurface {
            self_id: self.id,
            id,
            surface,
            parent,
        })?;
        self.tran.sync().await;
        let ss = Rc::new(TestSubsurface {
            id,
            tran: self.tran.clone(),
            destroyed: Cell::new(false),
            _server: self.tran.get_server_obj(id)?,
        });
        self.tran.add_obj(ss.clone())?;
        Ok(ss)
    }
}

impl Drop for TestSubcompositor {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestSubcompositor, WlSubcompositor;
}

impl TestObject for TestSubcompositor {}
