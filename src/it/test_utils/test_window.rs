use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_ifs::{
                test_single_pixel_buffer_manager::TestSinglePixelBufferManager,
                test_surface::TestSurface, test_viewport::TestViewport,
                test_xdg_surface::TestXdgSurface, test_xdg_toplevel::TestXdgToplevel,
            },
        },
        theme::Color,
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestWindow {
    pub surface: Rc<TestSurface>,
    pub spbm: Rc<TestSinglePixelBufferManager>,
    pub viewport: Rc<TestViewport>,
    pub xdg: Rc<TestXdgSurface>,
    pub tl: Rc<TestXdgToplevel>,
    pub color: Cell<Color>,
}

impl TestWindow {
    pub async fn map(&self) -> Result<(), TestError> {
        let buffer = self.spbm.create_buffer(self.color.get())?;
        self.surface.attach(buffer.id)?;
        self.viewport.set_source(0, 0, 1, 1)?;
        self.viewport
            .set_destination(self.tl.width.get(), self.tl.height.get())?;
        self.xdg.ack_configure(self.xdg.last_serial.get())?;
        self.surface.commit()?;
        self.surface.tran.sync().await;
        Ok(())
    }

    pub async fn map2(&self) -> TestResult {
        self.map().await?;
        self.map().await
    }

    #[allow(dead_code)]
    pub fn set_color(&self, r: u8, g: u8, b: u8, a: u8) {
        self.color.set(Color::from_rgba_straight(r, g, b, a));
    }
}
