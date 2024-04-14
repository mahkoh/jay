use {
    crate::it::{
        test_error::{TestError, TestResult},
        test_ifs::{test_xdg_surface::TestXdgSurface, test_xdg_toplevel::TestXdgToplevel},
        test_utils::test_surface_ext::TestSurfaceExt,
    },
    std::rc::Rc,
};

pub struct TestWindow {
    pub surface: TestSurfaceExt,
    pub xdg: Rc<TestXdgSurface>,
    pub tl: Rc<TestXdgToplevel>,
}

impl TestWindow {
    pub async fn map(&self) -> Result<(), TestError> {
        self.xdg.ack_configure(self.xdg.last_serial.get())?;
        self.surface
            .map(self.tl.core.width.get(), self.tl.core.height.get())
            .await?;
        Ok(())
    }

    pub async fn map2(&self) -> TestResult {
        self.map().await?;
        self.map().await
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8, a: u8) {
        self.surface.set_color(r, g, b, a);
    }
}
