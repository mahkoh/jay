use {
    crate::{
        fixed::Fixed,
        it::{test_error::TestError, test_object::TestObject, test_transport::TestTransport},
        wire::{WpViewportId, wp_viewport::*},
    },
    std::rc::Rc,
};

pub struct TestViewport {
    pub id: WpViewportId,
    pub tran: Rc<TestTransport>,
}

impl TestViewport {
    pub fn destroy(&self) -> Result<(), TestError> {
        self.tran.send(Destroy { self_id: self.id })?;
        Ok(())
    }

    pub fn set_source(&self, x: i32, y: i32, width: i32, height: i32) -> Result<(), TestError> {
        self.tran.send(SetSource {
            self_id: self.id,
            x: Fixed::from_int(x),
            y: Fixed::from_int(y),
            width: Fixed::from_int(width),
            height: Fixed::from_int(height),
        })?;
        Ok(())
    }

    pub fn set_destination(&self, width: i32, height: i32) -> Result<(), TestError> {
        self.tran.send(SetDestination {
            self_id: self.id,
            width: width.max(1),
            height: height.max(1),
        })?;
        Ok(())
    }
}

impl Drop for TestViewport {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestViewport, WpViewport;
}

impl TestObject for TestViewport {}
