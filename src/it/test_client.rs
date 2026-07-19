use crate::cli::ScreenshotFormat;
use crate::cli::screenshot::buf_to_bytes;
use crate::client::Client;
use crate::globals::GlobalBase;
use crate::it::test_error::TestError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_compositor::TestCompositor;
use crate::it::test_ifs::test_cursor_shape_manager::TestCursorShapeManager;
use crate::it::test_ifs::test_data_device_manager::TestDataDeviceManager;
use crate::it::test_ifs::test_fifo_manager::TestFifoManager;
use crate::it::test_ifs::test_jay_compositor::TestJayCompositor;
use crate::it::test_ifs::test_keyboard::TestKeyboard;
use crate::it::test_ifs::test_pointer::TestPointer;
use crate::it::test_ifs::test_pointer_warp::TestPointerWarp;
use crate::it::test_ifs::test_registry::TestRegistry;
use crate::it::test_ifs::test_seat::TestSeat;
use crate::it::test_ifs::test_session::TestSession;
use crate::it::test_ifs::test_shm::TestShm;
use crate::it::test_ifs::test_single_pixel_buffer_manager::TestSinglePixelBufferManager;
use crate::it::test_ifs::test_subcompositor::TestSubcompositor;
use crate::it::test_ifs::test_toplevel_session::TestToplevelSession;
use crate::it::test_ifs::test_viewporter::TestViewporter;
use crate::it::test_ifs::test_xdg_activation::TestXdgActivation;
use crate::it::test_ifs::test_xdg_base::TestXdgWmBase;
use crate::it::test_transport::TestTransport;
use crate::it::test_utils::test_surface_ext::TestSurfaceExt;
use crate::it::test_utils::test_window::TestWindow;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestClient {
    pub run: Rc<TestRun>,
    pub server: Rc<Client>,
    pub tran: Rc<TestTransport>,
    pub registry: Rc<TestRegistry>,
    pub jc: Rc<TestJayCompositor>,
    pub comp: Rc<TestCompositor>,
    pub sub: Rc<TestSubcompositor>,
    pub shm: Rc<TestShm>,
    pub spbm: Rc<TestSinglePixelBufferManager>,
    pub viewporter: Rc<TestViewporter>,
    pub xdg: Rc<TestXdgWmBase>,
    pub activation: Rc<TestXdgActivation>,
    pub data_device_manager: Rc<TestDataDeviceManager>,
    pub cursor_shape_manager: Rc<TestCursorShapeManager>,
    pub fifo_manager: Rc<TestFifoManager>,
    pub pointer_warp: Rc<TestPointerWarp>,
}

pub struct DefaultSeat {
    pub seat: Rc<TestSeat>,
    pub kb: Rc<TestKeyboard>,
    pub pointer: Rc<TestPointer>,
}

impl TestClient {
    #[expect(dead_code)]
    pub fn error(&self, msg: &str) {
        self.tran.error(msg)
    }

    pub async fn get_default_seat(&self) -> TestResult<DefaultSeat> {
        self.tran.sync().await;
        let seat = 'get_seat: {
            for seat in self.tran.run.state.globals.seats.lock().values() {
                if seat.seat_name() == "default" {
                    break 'get_seat seat.clone();
                }
            }
            bail!("Default seat not found");
        };
        let id = self.tran.id();
        let tseat = Rc::new(TestSeat {
            id,
            tran: self.tran.clone(),
            server: Default::default(),
            destroyed: Default::default(),
            caps: Cell::new(0),
            name: Default::default(),
        });
        self.registry.bind(&tseat, seat.name().raw(), 9)?;
        self.tran.sync().await;
        let server = self.tran.get_server_obj(tseat.id)?;
        tseat.server.set(Some(server));
        let pointer = tseat.get_pointer().await?;
        let tkb = tseat.get_keyboard().await?;
        Ok(DefaultSeat {
            seat: tseat,
            kb: tkb,
            pointer,
        })
    }

    pub async fn sync(&self) {
        self.run.sync().await;
        self.tran.sync().await;
        self.run.state.idle().await;
    }

    pub async fn take_screenshot(&self, include_cursor: bool) -> Result<Vec<u8>, TestError> {
        let (dmabuf, dev) = self.jc.take_screenshot(include_cursor).await?;
        let qoi = buf_to_bytes(
            &self.run.state.eventfd_cache,
            dev.as_ref(),
            &dmabuf,
            ScreenshotFormat::Qoi,
        )?;
        Ok(qoi)
    }

    #[expect(dead_code)]
    pub async fn save_screenshot(&self, name: &str, include_cursor: bool) -> Result<(), TestError> {
        let qoi = self.take_screenshot(include_cursor).await?;
        let path = format!("{}/screenshot_{}.qoi", self.run.out_dir, name);
        std::fs::write(path, qoi)?;
        Ok(())
    }

    pub async fn compare_screenshot(
        &self,
        name: &str,
        include_cursor: bool,
    ) -> Result<(), TestError> {
        let actual = self.take_screenshot(include_cursor).await?;
        let expected_path = format!("{}/screenshot_{}.qoi", self.run.in_dir, name);
        let expected = std::fs::read(expected_path)?;
        if actual != expected {
            let actual_out_path = format!("{}/screenshot_{}_actual.qoi", self.run.out_dir, name);
            let expected_out_path =
                format!("{}/screenshot_{}_expected.qoi", self.run.out_dir, name);
            let _ = std::fs::write(actual_out_path, actual);
            let _ = std::fs::write(expected_out_path, expected);
            bail!("Screenshots differ");
        }
        Ok(())
    }

    pub async fn create_surface_ext(self: &Rc<Self>) -> Result<TestSurfaceExt, TestError> {
        let surface = self.comp.create_surface().await?;
        let viewport = self.viewporter.get_viewport(&surface)?;
        Ok(TestSurfaceExt {
            client: self.clone(),
            surface,
            spbm: self.spbm.clone(),
            viewport,
            color: Cell::new(Color::SOLID_BLACK),
        })
    }

    pub async fn create_window_no_commit(self: &Rc<Self>) -> Result<Rc<TestWindow>, TestError> {
        let surface = self.create_surface_ext().await?;
        let xdg = self.xdg.create_xdg_surface(surface.surface.id).await?;
        let tl = xdg.create_toplevel().await?;
        Ok(Rc::new(TestWindow { surface, xdg, tl }))
    }

    pub async fn create_window(self: &Rc<Self>) -> Result<Rc<TestWindow>, TestError> {
        let win = self.create_window_no_commit().await?;
        win.surface.surface.commit()?;
        self.sync().await;
        Ok(win)
    }

    pub async fn restore_window(
        self: &Rc<Self>,
        session: &TestSession,
        name: &str,
    ) -> Result<(Rc<TestWindow>, Rc<TestToplevelSession>), TestError> {
        let win = self.create_window_no_commit().await?;
        let session = session.restore_toplevel(&win, name)?;
        win.surface.surface.commit()?;
        win.tl.core.configured().await;
        Ok((win, session))
    }
}

impl Drop for TestClient {
    fn drop(&mut self) {
        self.tran.kill();
    }
}
