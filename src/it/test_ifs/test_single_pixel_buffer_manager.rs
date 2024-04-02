use {
    crate::{
        it::{
            test_error::TestResult, test_ifs::test_buffer::TestBuffer, test_object::TestObject,
            test_transport::TestTransport,
        },
        theme::Color,
        wire::{wp_single_pixel_buffer_manager_v1::*, WpSinglePixelBufferManagerV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestSinglePixelBufferManager {
    pub id: WpSinglePixelBufferManagerV1Id,
    pub tran: Rc<TestTransport>,
}

impl TestSinglePixelBufferManager {
    pub fn create_buffer(&self, color: Color) -> TestResult<Rc<TestBuffer>> {
        let obj = Rc::new(TestBuffer {
            id: self.tran.id(),
            tran: self.tran.clone(),
            released: Cell::new(true),
            destroyed: Cell::new(false),
        });
        let map = |c: f32| (c as f64 * u32::MAX as f64) as u32;
        self.tran.send(CreateU32RgbaBuffer {
            self_id: self.id,
            id: obj.id,
            r: map(color.r),
            g: map(color.g),
            b: map(color.b),
            a: map(color.a),
        })?;
        self.tran.add_obj(obj.clone())?;
        Ok(obj)
    }
}

test_object! {
    TestSinglePixelBufferManager, WpSinglePixelBufferManagerV1;
}

impl TestObject for TestSinglePixelBufferManager {}
