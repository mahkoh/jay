use crate::it::test_client::TestClient;
use crate::it::test_error::TestError;
use crate::it::test_ifs::test_single_pixel_buffer_manager::TestSinglePixelBufferManager;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_ifs::test_viewport::TestViewport;
use crate::theme::Color;
use std::cell::Cell;
use std::ops::Deref;
use std::rc::Rc;

pub struct TestSurfaceExt {
    pub client: Rc<TestClient>,
    pub surface: Rc<TestSurface>,
    pub spbm: Rc<TestSinglePixelBufferManager>,
    pub viewport: Rc<TestViewport>,
    pub color: Cell<Color>,
}

impl Deref for TestSurfaceExt {
    type Target = TestSurface;

    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}

impl TestSurfaceExt {
    pub async fn map(&self, width: i32, height: i32) -> Result<(), TestError> {
        let buffer = self.spbm.create_buffer(self.color.get())?;
        self.surface.attach(buffer.id)?;
        self.viewport.set_source(0, 0, 1, 1)?;
        self.viewport.set_destination(width, height)?;
        self.surface.commit()?;
        self.client.sync().await;
        Ok(())
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8, a: u8) {
        self.color.set(Color::from_srgba_straight(r, g, b, a));
    }
}
