use crate::client::Client;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestError;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_ifs::test_viewport::TestViewport;
use crate::theme::Color;
use std::cell::Cell;
use std::ops::Deref;
use std::rc::Rc;

pub struct TestSurfaceExt {
    pub client: Rc<Client>,
    pub surface: Rc<TestSurface>,
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
        let buffer = self.client.create_single_pixel_buffer(self.color.get());
        self.surface.attach(buffer.id);
        self.viewport.set_source(0, 0, 1, 1);
        self.viewport.set_destination(width, height);
        self.surface.commit();
        self.client.sync().await;
        Ok(())
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8, a: u8) {
        self.color.set(Color::from_srgba_straight(r, g, b, a));
    }
}
