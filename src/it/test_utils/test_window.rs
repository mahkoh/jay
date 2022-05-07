use {
    crate::{
        format::ARGB8888,
        it::{
            test_error::{TestError, TestResult},
            test_ifs::{
                test_shm_buffer::TestShmBuffer, test_shm_pool::TestShmPool,
                test_surface::TestSurface, test_xdg_surface::TestXdgSurface,
                test_xdg_toplevel::TestXdgToplevel,
            },
        },
        theme::Color,
        utils::clonecell::CloneCell,
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestWindow {
    pub surface: Rc<TestSurface>,
    pub xdg: Rc<TestXdgSurface>,
    pub tl: Rc<TestXdgToplevel>,
    pub shm: Rc<TestShmPool>,
    pub buffer: CloneCell<Rc<TestShmBuffer>>,
    pub color: Cell<Color>,
}

impl TestWindow {
    pub async fn map(&self) -> Result<(), TestError> {
        let width = self.tl.width.get();
        let height = self.tl.height.get();
        let stride = width * 4;
        let size = (stride * height) as usize;
        self.shm.resize(size)?;
        let buffer = self.shm.create_buffer(0, width, height, stride, ARGB8888)?;
        buffer.fill(self.color.get());
        self.surface.attach(buffer.id)?;
        self.xdg.ack_configure(self.xdg.last_serial.get())?;
        self.surface.commit()?;
        self.buffer.set(buffer);
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
