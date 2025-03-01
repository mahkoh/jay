use {
    crate::{
        cmm::cmm_transfer_function::TransferFunction,
        it::{
            test_error::TestResult, test_ifs::test_buffer::TestBuffer, test_object::TestObject,
            test_transport::TestTransport,
        },
        theme::Color,
        wire::{WpSinglePixelBufferManagerV1Id, wp_single_pixel_buffer_manager_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestSinglePixelBufferManager {
    pub id: WpSinglePixelBufferManagerV1Id,
    pub tran: Rc<TestTransport>,
}

impl TestSinglePixelBufferManager {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
        }
    }

    pub fn create_buffer(&self, color: Color) -> TestResult<Rc<TestBuffer>> {
        let obj = Rc::new(TestBuffer {
            id: self.tran.id(),
            tran: self.tran.clone(),
            released: Cell::new(true),
            destroyed: Cell::new(false),
        });
        let map = |c: f32| (c as f64 * u32::MAX as f64) as u32;
        let [r, g, b, a] = color.to_array(TransferFunction::Srgb);
        self.tran.send(CreateU32RgbaBuffer {
            self_id: self.id,
            id: obj.id,
            r: map(r),
            g: map(g),
            b: map(b),
            a: map(a),
        })?;
        self.tran.add_obj(obj.clone())?;
        Ok(obj)
    }
}

test_object! {
    TestSinglePixelBufferManager, WpSinglePixelBufferManagerV1;
}

impl TestObject for TestSinglePixelBufferManager {}
