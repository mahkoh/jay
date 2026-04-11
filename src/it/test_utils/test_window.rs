use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_ifs::{test_xdg_surface::TestXdgSurface, test_xdg_toplevel::TestXdgToplevel},
            test_utils::test_surface_ext::TestSurfaceExt,
        },
        tree::{
            ToplevelNodeBase, WorkspaceNode, WorkspaceNodeId, toplevel_set_floating,
            toplevel_set_workspace,
        },
    },
    std::rc::Rc,
};

pub struct TestWindow {
    pub surface: TestSurfaceExt,
    pub tl: Rc<TestXdgToplevel>,
    pub xdg: Rc<TestXdgSurface>,
}

impl TestWindow {
    pub async fn map(&self) -> Result<(), TestError> {
        if let Some(serial) = self.xdg.last_serial.take() {
            self.xdg.ack_configure(serial)?;
        }
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

    pub fn set_workspace(&self, ws: &Rc<WorkspaceNode>) {
        toplevel_set_workspace(&self.tl.server.state, self.tl.server.clone(), ws);
    }

    pub fn set_floating(&self, floating: bool) {
        toplevel_set_floating(&self.tl.server.state, self.tl.server.clone(), floating);
    }

    pub fn workspace_id(&self) -> Option<WorkspaceNodeId> {
        self.tl.server.tl_data().workspace.get().map(|w| w.id)
    }
}
