use {
    crate::{
        ifs::wl_region::WlRegion,
        it::{test_error::TestError, test_object::TestObject, test_transport::TestTransport},
        rect::{Rect, RegionBuilder},
        wire::{WlRegionId, wl_region::*},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

pub struct TestRegion {
    pub id: WlRegionId,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub server: Rc<WlRegion>,
    pub expected: RefCell<RegionBuilder>,
}

impl TestRegion {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn add(&self, rect: Rect) -> Result<(), TestError> {
        self.expected.borrow_mut().add(rect);
        self.tran.send(Add {
            self_id: self.id,
            x: rect.x1(),
            y: rect.y1(),
            width: rect.width(),
            height: rect.height(),
        })?;
        Ok(())
    }

    pub fn subtract(&self, rect: Rect) -> Result<(), TestError> {
        self.expected.borrow_mut().sub(rect);
        self.tran.send(Subtract {
            self_id: self.id,
            x: rect.x1(),
            y: rect.y1(),
            width: rect.width(),
            height: rect.height(),
        })?;
        Ok(())
    }

    pub async fn check(&self) -> Result<(), TestError> {
        self.tran.sync().await;
        let expected = self.expected.borrow_mut().get();
        let actual = self.server.region();
        tassert_eq!(expected, actual);
        Ok(())
    }
}

impl Drop for TestRegion {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestRegion, WlRegion;
}

impl TestObject for TestRegion {}
