use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{
            Backend, BackendEvent, Connector, ConnectorEvent, ConnectorId, ConnectorKernelId,
            InputDevice, InputDeviceAccelProfile, InputDeviceCapability, InputDeviceId, InputEvent,
            Mode, MonitorInfo, TransformMatrix,
        },
        compositor::TestFuture,
        render::{RenderContext, RenderError},
        state::State,
        utils::{
            clonecell::CloneCell, copyhashmap::CopyHashMap, oserror::OsError, syncqueue::SyncQueue,
        },
        video::drm::{ConnectorType, Drm},
    },
    bstr::ByteSlice,
    std::{any::Any, cell::Cell, io, os::unix::ffi::OsStrExt, pin::Pin, rc::Rc},
    thiserror::Error,
    uapi::c,
};

#[derive(Debug, Error)]
pub enum TestBackendError {
    #[error("Could not read /dev/dri")]
    ReadDri(#[source] io::Error),
    #[error("There are no drm nodes in /dev/dri")]
    NoDrmNode,
    #[error("Could not open drm node {0}")]
    OpenDrmNode(String, #[source] OsError),
    #[error("Could not create a render context")]
    RenderContext(#[source] RenderError),
}

pub struct TestBackend {
    pub state: Rc<State>,
    pub test_future: TestFuture,
    pub default_connector: Rc<TestConnector>,
}

impl TestBackend {
    pub fn new(state: &Rc<State>, future: TestFuture) -> Self {
        let connector = Rc::new(TestConnector {
            id: state.connector_ids.next(),
            kernel_id: ConnectorKernelId {
                ty: ConnectorType::VGA,
                idx: 1,
            },
            events: Default::default(),
            on_change: Default::default(),
        });
        Self {
            state: state.clone(),
            test_future: future,
            default_connector: connector,
        }
    }

    pub fn install_default(&self) {
        self.state
            .backend_events
            .push(BackendEvent::NewConnector(self.default_connector.clone()));
        let mode = Mode {
            width: 800,
            height: 600,
            refresh_rate_millihz: 60_000,
        };
        self.default_connector
            .events
            .push(ConnectorEvent::Connected(MonitorInfo {
                modes: vec![mode],
                manufacturer: "jay".to_string(),
                product: "TestConnector".to_string(),
                serial_number: self.default_connector.id.to_string(),
                initial_mode: mode,
                width_mm: 80,
                height_mm: 60,
            }));
    }

    fn create_render_context(&self) -> Result<(), TestBackendError> {
        let dri = match std::fs::read_dir("/dev/dri") {
            Ok(d) => d,
            Err(e) => return Err(TestBackendError::ReadDri(e)),
        };
        let mut files = vec![];
        for f in dri {
            let f = match f {
                Ok(f) => f,
                Err(e) => return Err(TestBackendError::ReadDri(e)),
            };
            files.push(f.path());
        }
        let node = 'node: {
            for f in &files {
                if let Some(file) = f.file_name() {
                    if file.as_bytes().starts_with_str("renderD") {
                        break 'node f;
                    }
                }
            }
            for f in &files {
                if let Some(file) = f.file_name() {
                    if file.as_bytes().starts_with_str("card") {
                        break 'node f;
                    }
                }
            }
            return Err(TestBackendError::NoDrmNode);
        };
        let file = match uapi::open(node.as_path(), c::O_RDWR | c::O_CLOEXEC, 0) {
            Ok(f) => f,
            Err(e) => {
                return Err(TestBackendError::OpenDrmNode(
                    node.as_os_str().as_bytes().as_bstr().to_string(),
                    e.into(),
                ))
            }
        };
        let drm = Drm::open_existing(file);
        let ctx = match RenderContext::from_drm_device(&drm) {
            Ok(ctx) => ctx,
            Err(e) => return Err(TestBackendError::RenderContext(e)),
        };
        self.state.set_render_ctx(&Rc::new(ctx));
        Ok(())
    }
}

impl Backend for TestBackend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn std::error::Error>>> {
        let future = (self.test_future)(&self.state);
        let slf = self.clone();
        self.state.eng.spawn(async move {
            if let Err(e) = slf.create_render_context() {
                return Err(Box::new(e) as Box<_>);
            }
            let future: Pin<_> = future.into();
            future.await;
            Ok(())
        })
    }

    fn into_any(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn switch_to(&self, vtnr: u32) {
        let _ = vtnr;
    }

    fn set_idle(&self, _idle: bool) {}

    fn supports_idle(&self) -> bool {
        true
    }

    fn supports_presentation_feedback(&self) -> bool {
        true
    }
}

pub struct TestConnector {
    pub id: ConnectorId,
    pub kernel_id: ConnectorKernelId,
    pub events: SyncQueue<ConnectorEvent>,
    pub on_change: CloneCell<Option<Rc<dyn Fn()>>>,
}

impl Connector for TestConnector {
    fn id(&self) -> ConnectorId {
        self.id
    }

    fn kernel_id(&self) -> ConnectorKernelId {
        self.kernel_id
    }

    fn event(&self) -> Option<ConnectorEvent> {
        self.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change.set(Some(cb));
    }

    fn damage(&self) {
        // nothing
    }
}

pub struct TestInputDevice {
    pub id: InputDeviceId,
    pub remove: Cell<bool>,
    pub events: SyncQueue<InputEvent>,
    pub on_change: CloneCell<Option<Rc<dyn Fn()>>>,
    pub capabilities: CopyHashMap<InputDeviceCapability, ()>,
    pub transform_matrix: Cell<TransformMatrix>,
    pub name: Rc<String>,
    pub accel_speed: Cell<f64>,
    pub accel_profile: Cell<InputDeviceAccelProfile>,
    pub left_handed: Cell<bool>,
}

impl InputDevice for TestInputDevice {
    fn id(&self) -> InputDeviceId {
        self.id
    }

    fn removed(&self) -> bool {
        self.remove.get()
    }

    fn event(&self) -> Option<InputEvent> {
        self.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change.set(Some(cb));
    }

    fn grab(&self, _grab: bool) {
        // nothing
    }

    fn has_capability(&self, cap: InputDeviceCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    fn set_left_handed(&self, left_handed: bool) {
        self.left_handed.set(left_handed);
    }

    fn set_accel_profile(&self, profile: InputDeviceAccelProfile) {
        self.accel_profile.set(profile);
    }

    fn set_accel_speed(&self, speed: f64) {
        self.accel_speed.set(speed)
    }

    fn set_transform_matrix(&self, matrix: TransformMatrix) {
        self.transform_matrix.set(matrix);
    }

    fn name(&self) -> Rc<String> {
        self.name.clone()
    }
}
