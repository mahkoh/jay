use {
    crate::{
        ifs::wl_surface::wl_subsurface::WlSubsurface,
        it::{test_error::TestError, test_object::TestObject, test_transport::TestTransport},
        wire::{WlSubsurfaceId, WlSurfaceId, wl_subsurface::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestSubsurface {
    pub id: WlSubsurfaceId,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub _server: Rc<WlSubsurface>,
}

impl TestSubsurface {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn set_position(&self, x: i32, y: i32) -> Result<(), TestError> {
        self.tran.send(SetPosition {
            self_id: self.id,
            x,
            y,
        })
    }

    pub fn place_above(&self, surface: WlSurfaceId) -> Result<(), TestError> {
        self.tran.send(PlaceAbove {
            self_id: self.id,
            sibling: surface,
        })
    }

    pub fn place_below(&self, surface: WlSurfaceId) -> Result<(), TestError> {
        self.tran.send(PlaceBelow {
            self_id: self.id,
            sibling: surface,
        })
    }

    #[expect(dead_code)]
    pub fn set_sync(&self) -> Result<(), TestError> {
        self.tran.send(SetSync { self_id: self.id })
    }

    pub fn set_desync(&self) -> Result<(), TestError> {
        self.tran.send(SetDesync { self_id: self.id })
    }
}

impl Drop for TestSubsurface {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestSubsurface, WlSubsurface;
}

impl TestObject for TestSubsurface {}
