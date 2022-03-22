use crate::async_engine::SpawnedFuture;
use crate::backend::{
    Backend, BackendEvent, InputDevice, InputDeviceId, InputEvent, KeyState, Output, OutputId,
    ScrollAxis,
};
use crate::drm::drm::{Drm, DrmError};
use crate::drm::gbm::{GbmDevice, GbmError, GBM_BO_USE_RENDERING};
use crate::drm::{ModifiedFormat, INVALID_MODIFIER};
use crate::fixed::Fixed;
use crate::format::XRGB8888;
use crate::render::{Framebuffer, RenderContext, RenderError};
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::wire_xcon::{
    ChangeProperty, ChangeWindowAttributes, ConfigureNotify, CreateCursor, CreatePixmap,
    CreateWindow, CreateWindowValues, DestroyNotify, Dri3Open, Dri3PixmapFromBuffer,
    Dri3QueryVersion, Extension, FreePixmap, MapWindow, PresentCompleteNotify, PresentIdleNotify,
    PresentPixmap, PresentQueryVersion, PresentSelectInput, XiButtonPress, XiButtonRelease,
    XiDeviceInfo, XiEnter, XiEventMask, XiGetDeviceButtonMapping, XiGrabDevice, XiHierarchy,
    XiKeyPress, XiKeyRelease, XiMotion, XiQueryDevice, XiQueryVersion, XiSelectEvents,
    XiUngrabDevice, XkbPerClientFlags, XkbUseExtension,
};
use crate::xcon::consts::{
    ATOM_STRING, ATOM_WM_CLASS, EVENT_MASK_EXPOSURE, EVENT_MASK_STRUCTURE_NOTIFY,
    EVENT_MASK_VISIBILITY_CHANGE, GRAB_MODE_ASYNC, GRAB_STATUS_SUCCESS, INPUT_DEVICE_ALL,
    INPUT_DEVICE_ALL_MASTER, INPUT_DEVICE_TYPE_MASTER_KEYBOARD, INPUT_HIERARCHY_MASK_MASTER_ADDED,
    INPUT_HIERARCHY_MASK_MASTER_REMOVED, PRESENT_EVENT_MASK_COMPLETE_NOTIFY,
    PRESENT_EVENT_MASK_IDLE_NOTIFY, PROP_MODE_REPLACE, WINDOW_CLASS_INPUT_OUTPUT,
    XI_EVENT_MASK_BUTTON_PRESS, XI_EVENT_MASK_BUTTON_RELEASE, XI_EVENT_MASK_ENTER,
    XI_EVENT_MASK_FOCUS_IN, XI_EVENT_MASK_FOCUS_OUT, XI_EVENT_MASK_HIERARCHY,
    XI_EVENT_MASK_KEY_PRESS, XI_EVENT_MASK_KEY_RELEASE, XI_EVENT_MASK_LEAVE, XI_EVENT_MASK_MOTION,
    XI_EVENT_MASK_TOUCH_BEGIN, XI_EVENT_MASK_TOUCH_END, XI_EVENT_MASK_TOUCH_UPDATE,
    XKB_PER_CLIENT_FLAG_DETECTABLE_AUTO_REPEAT,
};
use crate::xcon::{Event, XEvent, Xcon, XconError};
use crate::{AsyncQueue, ErrorFmt, NumCell, Phase, State};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XorgngBackendError {
    #[error("Could not connect to the X server")]
    CannotConnect(#[source] XconError),
    #[error("Could not enable XInput")]
    EnableXinput(#[source] XconError),
    #[error("Could not enable Present")]
    EnablePresent(#[source] XconError),
    #[error("Could not enable XKB")]
    EnableXkb(#[source] XconError),
    #[error("Could not enable DRI3")]
    EnableDri3(#[source] XconError),
    #[error("DriOpen returned an error")]
    DriOpen(#[source] XconError),
    #[error("Could not create a pixmap")]
    CreatePixmap(#[source] XconError),
    #[error("Could not create a cursor")]
    CreateCursor(#[source] XconError),
    #[error("Could not select XInput hierarchy events")]
    SelectHierarchyEvents(#[source] XconError),
    #[error("The drm subsystem returned an error")]
    DrmError(#[from] DrmError),
    #[error("The gbm subsystem returned an error")]
    GbmError(#[from] GbmError),
    #[error("Could not import a dma-buf")]
    ImportBuffer(#[source] XconError),
    #[error("Could not create an EGL context")]
    CreateEgl(#[source] RenderError),
    #[error("Could not create a framebuffer from a dma-buf")]
    CreateFramebuffer(#[source] RenderError),
    #[error("Could not select input events")]
    CannotSelectInputEvents(#[source] XconError),
    #[error("Could not select present events")]
    CannotSelectPresentEvents(#[source] XconError),
    #[error("libloading returned an error")]
    Libloading(#[from] libloading::Error),
    #[error("An unspecified X error occurred")]
    XconError(#[from] XconError),
    #[error("Could not create a window")]
    CreateWindow(#[source] XconError),
    #[error("Could not set WM_CLASS")]
    WmClass(#[source] XconError),
    #[error("Could not select window events")]
    WindowEvents(#[source] XconError),
    #[error("Could not map a window")]
    MapWindow(#[source] XconError),
    #[error("Could not query device")]
    QueryDevice(#[source] XconError),
}

pub struct XorgngBackend {
    _data: Rc<XorgngBackendData>,
    _events: SpawnedFuture<()>,
    _present: SpawnedFuture<()>,
    _grab: SpawnedFuture<()>,
}

impl Backend for XorgngBackend {
    fn switch_to(&self, _vtnr: u32) {
        log::error!("Xorg backend cannot switch vts");
    }
}

pub struct XorgngBackendData {
    state: Rc<State>,
    c: Rc<Xcon>,
    outputs: CopyHashMap<u32, Rc<XorgOutput>>,
    seats: CopyHashMap<u16, Rc<XorgSeat>>,
    mouse_seats: CopyHashMap<u16, Rc<XorgSeat>>,
    ctx: Rc<RenderContext>,
    gbm: GbmDevice,
    cursor: u32,
    root: u32,
    scheduled_present: AsyncQueue<Rc<XorgOutput>>,
    grab_requests: AsyncQueue<(Rc<XorgSeat>, bool)>,
}

impl XorgngBackend {
    pub async fn run(state: &Rc<State>) -> Result<Rc<Self>, XorgngBackendError> {
        let c = match Xcon::connect(state.eng.clone()).await {
            Ok(c) => c,
            Err(e) => return Err(XorgngBackendError::CannotConnect(e)),
        };
        if let Err(e) = c
            .call(&XiQueryVersion {
                major_version: 2,
                minor_version: 2,
            })
            .await
        {
            return Err(XorgngBackendError::EnableXinput(e));
        }
        if let Err(e) = c
            .call(&Dri3QueryVersion {
                major_version: 1,
                minor_version: 0,
            })
            .await
        {
            return Err(XorgngBackendError::EnableDri3(e));
        }
        if let Err(e) = c
            .call(&PresentQueryVersion {
                major_version: 1,
                minor_version: 0,
            })
            .await
        {
            return Err(XorgngBackendError::EnablePresent(e));
        }
        if let Err(e) = c
            .call(&XkbUseExtension {
                wanted_major: 1,
                wanted_minor: 0,
            })
            .await
        {
            return Err(XorgngBackendError::EnableXkb(e));
        }
        let root = c.setup().screens[0].root;
        let drm = {
            let res = c
                .call(&Dri3Open {
                    drawable: root,
                    provider: 0,
                })
                .await;
            match res {
                Ok(r) => Drm::reopen(r.get().device_fd.raw(), false)?,
                Err(e) => return Err(XorgngBackendError::DriOpen(e)),
            }
        };
        let gbm = GbmDevice::new(&drm)?;
        let ctx = match RenderContext::from_drm_device(&drm) {
            Ok(r) => Rc::new(r),
            Err(e) => return Err(XorgngBackendError::CreateEgl(e)),
        };
        let cursor = {
            let cp = CreatePixmap {
                depth: 1,
                pid: c.generate_id()?,
                drawable: root,
                width: 1,
                height: 1,
            };
            if let Err(e) = c.call(&cp).await {
                return Err(XorgngBackendError::CreatePixmap(e));
            }
            let cc = CreateCursor {
                cid: c.generate_id()?,
                source: cp.pid,
                mask: cp.pid,
                fore_red: 0,
                fore_green: 0,
                fore_blue: 0,
                back_red: 0,
                back_green: 0,
                back_blue: 0,
                x: 0,
                y: 0,
            };
            if let Err(e) = c.call(&cc).await {
                return Err(XorgngBackendError::CreateCursor(e));
            }
            c.call(&FreePixmap { pixmap: cp.pid });
            cc.cid
        };
        {
            let se = XiSelectEvents {
                window: c.setup().screens[0].root,
                masks: Cow::Borrowed(&[XiEventMask {
                    deviceid: INPUT_DEVICE_ALL,
                    mask: &[XI_EVENT_MASK_HIERARCHY],
                }]),
            };
            if let Err(e) = c.call(&se).await {
                return Err(XorgngBackendError::SelectHierarchyEvents(e));
            }
        }

        let data = Rc::new(XorgngBackendData {
            state: state.clone(),
            c,
            outputs: Default::default(),
            seats: Default::default(),
            mouse_seats: Default::default(),
            ctx: ctx.clone(),
            gbm,
            cursor,
            root,
            scheduled_present: Default::default(),
            grab_requests: Default::default(),
        });
        data.add_output().await?;
        data.query_devices(INPUT_DEVICE_ALL_MASTER).await?;

        let slf = Rc::new(Self {
            _events: state.eng.spawn(data.clone().event_handler()),
            _grab: state.eng.spawn(data.clone().grab_handler()),
            _present: state
                .eng
                .spawn2(Phase::Present, data.clone().present_handler()),
            _data: data,
        });

        state.set_render_ctx(&ctx);
        state.backend.set(Some(slf.clone()));

        Ok(slf)
    }
}

impl XorgngBackendData {
    async fn event_handler(self: Rc<Self>) {
        loop {
            let event = self.c.event().await;
            if let Err(e) = self.handle_event(&event).await {
                log::error!(
                    "Fatal error: Could not handle an event from the X server: {}",
                    ErrorFmt(e)
                );
                self.state.el.stop();
                return;
            }
        }
    }

    async fn present_handler(self: Rc<Self>) {
        loop {
            let output = self.scheduled_present.pop().await;
            self.present(&output).await;
        }
    }

    async fn grab_handler(self: Rc<Self>) {
        loop {
            let (dev, grab) = self.grab_requests.pop().await;
            self.handle_grab_request(&dev, grab).await;
        }
    }

    async fn handle_grab_request(&self, dev: &XorgSeat, grab: bool) {
        if grab {
            let xg = XiGrabDevice {
                window: self.root,
                time: 0,
                cursor: 0,
                deviceid: dev.kb,
                mode: GRAB_MODE_ASYNC,
                paired_device_mode: GRAB_MODE_ASYNC,
                owner_events: 1,
                mask: &[],
            };
            let res = match self.c.call(&xg).await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Could not grab device {}: {}", dev.kb, ErrorFmt(e));
                    return;
                }
            };
            let res = res.get();
            if res.status != GRAB_STATUS_SUCCESS {
                log::error!("Could not grab device {}: status = {}", dev.kb, res.status);
            }
        } else {
            let ug = XiUngrabDevice {
                time: 0,
                deviceid: dev.kb,
            };
            if let Err(e) = self.c.call(&ug).await {
                log::error!("Could not ungrab device {}: {}", dev.kb, ErrorFmt(e));
            }
        }
    }

    async fn create_images(
        &self,
        window: u32,
        width: i32,
        height: i32,
    ) -> Result<[XorgImage; 2], XorgngBackendError> {
        let format = ModifiedFormat {
            format: XRGB8888,
            modifier: INVALID_MODIFIER,
        };
        let mut images = [None, None];
        for i in 0..2 {
            let bo = self
                .gbm
                .create_bo(width, height, &format, GBM_BO_USE_RENDERING)?;
            let dma = bo.dma();
            assert!(dma.planes.len() == 1);
            let plane = dma.planes.first().unwrap();
            let size = plane.stride * dma.height as u32;
            let fb = match self.ctx.dmabuf_fb(dma) {
                Ok(f) => f,
                Err(e) => return Err(XorgngBackendError::CreateFramebuffer(e)),
            };
            let pixmap = {
                let pfb = Dri3PixmapFromBuffer {
                    pixmap: self.c.generate_id()?,
                    drawable: window,
                    size,
                    width: dma.width as _,
                    height: dma.height as _,
                    stride: plane.stride as _,
                    depth: 24,
                    bpp: 32,
                    pixmap_fd: plane.fd.clone(),
                };
                if let Err(e) = self.c.call(&pfb).await {
                    return Err(XorgngBackendError::ImportBuffer(e));
                }
                pfb.pixmap
            };
            images[i] = Some(XorgImage {
                pixmap: Cell::new(pixmap),
                fb: CloneCell::new(fb),
                idle: Cell::new(true),
                render_on_idle: Cell::new(false),
                last_serial: Cell::new(0),
            });
        }
        Ok([images[0].take().unwrap(), images[1].take().unwrap()])
    }

    async fn add_output(self: &Rc<Self>) -> Result<(), XorgngBackendError> {
        const WIDTH: i32 = 800;
        const HEIGHT: i32 = 600;
        let window_id = {
            let cw = CreateWindow {
                depth: 0,
                wid: self.c.generate_id()?,
                parent: self.root,
                x: 0,
                y: 0,
                width: WIDTH as _,
                height: HEIGHT as _,
                border_width: 0,
                class: WINDOW_CLASS_INPUT_OUTPUT,
                visual: 0,
                values: Default::default(),
            };
            if let Err(e) = self.c.call(&cw).await {
                return Err(XorgngBackendError::CreateWindow(e));
            }
            cw.wid
        };
        let images = self.create_images(window_id, WIDTH, HEIGHT).await?;
        let output = Rc::new(XorgOutput {
            id: self.state.output_ids.next(),
            _backend: self.clone(),
            window: window_id,
            removed: Cell::new(false),
            width: Cell::new(0),
            height: Cell::new(0),
            serial: Default::default(),
            next_msc: Cell::new(0),
            next_image: Default::default(),
            cb: CloneCell::new(None),
            images,
        });
        {
            let class = "jay\0jay\0";
            let cp = ChangeProperty {
                mode: PROP_MODE_REPLACE,
                window: window_id,
                property: ATOM_WM_CLASS,
                ty: ATOM_STRING,
                format: 8,
                data: class.as_bytes(),
            };
            if let Err(e) = self.c.call(&cp).await {
                return Err(XorgngBackendError::WmClass(e));
            };
        }
        {
            let cwa = ChangeWindowAttributes {
                window: window_id,
                values: CreateWindowValues {
                    event_mask: Some(
                        EVENT_MASK_EXPOSURE
                            | EVENT_MASK_STRUCTURE_NOTIFY
                            | EVENT_MASK_VISIBILITY_CHANGE,
                    ),
                    cursor: Some(self.cursor),
                    ..Default::default()
                },
            };
            if let Err(e) = self.c.call(&cwa).await {
                return Err(XorgngBackendError::WindowEvents(e));
            }
        }
        if let Err(e) = self.c.call(&MapWindow { window: window_id }).await {
            return Err(XorgngBackendError::MapWindow(e));
        }
        {
            let mask = 0
                | XI_EVENT_MASK_MOTION
                | XI_EVENT_MASK_BUTTON_PRESS
                | XI_EVENT_MASK_BUTTON_RELEASE
                | XI_EVENT_MASK_KEY_PRESS
                | XI_EVENT_MASK_KEY_RELEASE
                | XI_EVENT_MASK_ENTER
                | XI_EVENT_MASK_LEAVE
                | XI_EVENT_MASK_FOCUS_IN
                | XI_EVENT_MASK_FOCUS_OUT
                | XI_EVENT_MASK_TOUCH_BEGIN
                | XI_EVENT_MASK_TOUCH_UPDATE
                | XI_EVENT_MASK_TOUCH_END;
            let mask = [XiEventMask {
                deviceid: INPUT_DEVICE_ALL_MASTER,
                mask: &[mask],
            }];
            let xs = XiSelectEvents {
                window: window_id,
                masks: Cow::Borrowed(&mask[..]),
            };
            if let Err(e) = self.c.call(&xs).await {
                return Err(XorgngBackendError::CannotSelectInputEvents(e));
            }
        }
        {
            let mask = 0 | PRESENT_EVENT_MASK_IDLE_NOTIFY | PRESENT_EVENT_MASK_COMPLETE_NOTIFY;
            let si = PresentSelectInput {
                eid: self.c.generate_id()?,
                window: window_id,
                event_mask: mask,
            };
            if let Err(e) = self.c.call(&si).await {
                return Err(XorgngBackendError::CannotSelectPresentEvents(e));
            }
        }
        self.outputs.set(window_id, output.clone());
        self.state
            .backend_events
            .push(BackendEvent::NewOutput(output.clone()));
        self.present(&output).await;
        Ok(())
    }

    async fn query_devices(self: &Rc<Self>, deviceid: u16) -> Result<(), XorgngBackendError> {
        let reply = match self.c.call(&XiQueryDevice { deviceid }).await {
            Ok(r) => r,
            Err(e) => return Err(XorgngBackendError::QueryDevice(e)),
        };
        for dev in reply.get().infos.iter() {
            self.handle_input_device(dev).await;
        }
        Ok(())
    }

    async fn handle_input_device(self: &Rc<Self>, info: &XiDeviceInfo<'_>) {
        if info.ty != INPUT_DEVICE_TYPE_MASTER_KEYBOARD {
            return;
        }
        self.mouse_seats.remove(&info.attachment);
        if let Some(kb) = self.seats.remove(&info.deviceid) {
            kb.removed.set(true);
            kb.kb_changed();
            kb.mouse_changed();
        }
        let pcf = XkbPerClientFlags {
            device_spec: info.deviceid,
            change: XKB_PER_CLIENT_FLAG_DETECTABLE_AUTO_REPEAT,
            value: XKB_PER_CLIENT_FLAG_DETECTABLE_AUTO_REPEAT,
            ctrls_to_change: 0,
            auto_ctrls: 0,
            auto_ctrls_values: 0,
        };
        if let Err(e) = self.c.call(&pcf).await {
            log::warn!(
                "Could not make auto repeat detectable for keyboard {}: {}",
                info.deviceid,
                ErrorFmt(e),
            );
        }
        let seat = Rc::new(XorgSeat {
            kb_id: self.state.input_device_ids.next(),
            mouse_id: self.state.input_device_ids.next(),
            backend: self.clone(),
            kb: info.deviceid,
            mouse: info.attachment,
            removed: Cell::new(false),
            kb_cb: Default::default(),
            mouse_cb: Default::default(),
            kb_events: RefCell::new(Default::default()),
            mouse_events: RefCell::new(Default::default()),
            button_map: Default::default(),
        });
        seat.update_button_map().await;
        self.seats.set(info.deviceid, seat.clone());
        self.mouse_seats.set(info.attachment, seat.clone());
        self.state
            .backend_events
            .push(BackendEvent::NewInputDevice(Rc::new(XorgSeatMouse(
                seat.clone(),
            ))));
        self.state
            .backend_events
            .push(BackendEvent::NewInputDevice(Rc::new(XorgSeatKeyboard(
                seat.clone(),
            ))));
    }

    async fn handle_event(self: &Rc<Self>, event: &Event) -> Result<(), XorgngBackendError> {
        match event.ext() {
            Some(ext) => self.handle_ext_event(ext, event).await,
            _ => self.handle_core_event(event).await,
        }
    }

    async fn handle_ext_event(
        self: &Rc<Self>,
        ext: Extension,
        event: &Event,
    ) -> Result<(), XorgngBackendError> {
        match ext {
            Extension::Present => self.handle_present_event(event),
            Extension::XInputExtension => self.handle_input_event(event).await,
            _ => Ok(()),
        }
    }

    async fn handle_core_event(self: &Rc<Self>, event: &Event) -> Result<(), XorgngBackendError> {
        match event.code() {
            ConfigureNotify::OPCODE => self.handle_configure(event).await,
            DestroyNotify::OPCODE => self.handle_destroy(event),
            _ => Ok(()),
        }
    }

    fn handle_present_event(self: &Rc<Self>, event: &Event) -> Result<(), XorgngBackendError> {
        match event.code() {
            PresentCompleteNotify::OPCODE => self.handle_present_complete(event)?,
            PresentIdleNotify::OPCODE => self.handle_present_idle(event)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_present_complete(self: &Rc<Self>, event: &Event) -> Result<(), XorgngBackendError> {
        let event: PresentCompleteNotify = event.parse()?;
        let window = event.window;
        let output = match self.outputs.get(&window) {
            Some(o) => o,
            _ => return Ok(()),
        };
        output.next_msc.set(event.msc + 1);
        let image = &output.images[output.next_image.get() % output.images.len()];
        if image.idle.get() {
            self.schedule_present(&output);
        } else {
            image.render_on_idle.set(true);
        }
        Ok(())
    }

    fn handle_present_idle(self: &Rc<Self>, event: &Event) -> Result<(), XorgngBackendError> {
        let event: PresentIdleNotify = event.parse()?;
        let output = match self.outputs.get(&event.window) {
            Some(o) => o,
            _ => return Ok(()),
        };
        for image in &output.images {
            if image.last_serial.get() == event.serial {
                image.idle.set(true);
                if image.render_on_idle.replace(false) {
                    self.schedule_present(&output);
                }
            }
        }
        Ok(())
    }

    fn schedule_present(&self, output: &Rc<XorgOutput>) {
        self.scheduled_present.push(output.clone());
    }

    async fn present(&self, output: &Rc<XorgOutput>) {
        if output.removed.get() {
            return;
        }

        let image = &output.images[output.next_image.fetch_add(1) % output.images.len()];
        let serial = output.serial.fetch_add(1);

        if let Some(node) = self.state.root.outputs.get(&output.id) {
            let fb = image.fb.get();
            fb.render(&*node, &self.state, Some(node.position.get()));
        }

        let pp = PresentPixmap {
            window: output.window,
            pixmap: image.pixmap.get(),
            serial,
            valid: 0,
            update: 0,
            x_off: 0,
            y_off: 0,
            target_crtc: 0,
            wait_fence: 0,
            idle_fence: 0,
            options: 0,
            target_msc: output.next_msc.get(),
            divisor: 1,
            remainder: 0,
            notifies: Default::default(),
        };
        if let Err(e) = self.c.call(&pp).await {
            log::error!("Could not present image: {:?}", e);
            return;
        }
        image.idle.set(false);
        image.last_serial.set(serial);
    }

    async fn handle_input_event(self: &Rc<Self>, event: &Event) -> Result<(), XorgngBackendError> {
        match event.code() {
            XiMotion::OPCODE => self.handle_input_motion(event),
            XiEnter::OPCODE => self.handle_input_enter(event),
            XiButtonPress::OPCODE => self.handle_input_button_press(event, KeyState::Pressed),
            XiButtonRelease::OPCODE => self.handle_input_button_press(event, KeyState::Released),
            XiKeyPress::OPCODE => self.handle_input_key_press(event, KeyState::Pressed),
            XiKeyRelease::OPCODE => self.handle_input_key_press(event, KeyState::Released),
            XiHierarchy::OPCODE => self.handle_input_hierarchy(event).await,
            _ => Ok(()),
        }
    }

    fn handle_input_button_press(
        self: &Rc<Self>,
        event: &Event,
        state: KeyState,
    ) -> Result<(), XorgngBackendError> {
        let event: XiButtonPress = event.parse()?;
        if let Some(seat) = self.mouse_seats.get(&event.deviceid) {
            let button = event.detail;
            // let button = seat.button_map.get(&event.detail).unwrap_or(event.detail);
            if matches!(button, 4..=7) {
                if state == KeyState::Pressed {
                    let (axis, val) = match button {
                        4 => (ScrollAxis::Vertical, -15),
                        5 => (ScrollAxis::Vertical, 15),
                        6 => (ScrollAxis::Horizontal, -15),
                        7 => (ScrollAxis::Horizontal, 15),
                        _ => unreachable!(),
                    };
                    seat.mouse_event(InputEvent::Scroll(val, axis));
                }
            } else {
                const BTN_LEFT: u32 = 0x110;
                const BTN_RIGHT: u32 = 0x111;
                const BTN_MIDDLE: u32 = 0x112;
                const BTN_SIDE: u32 = 0x113;
                let button = match button {
                    0 => return Ok(()),
                    1 => BTN_LEFT,
                    2 => BTN_MIDDLE,
                    3 => BTN_RIGHT,
                    n => BTN_SIDE + n - 8,
                };
                seat.mouse_event(InputEvent::Button(button, state));
            }
        }
        Ok(())
    }

    fn handle_input_key_press(
        self: &Rc<Self>,
        event: &Event,
        state: KeyState,
    ) -> Result<(), XorgngBackendError> {
        let event: XiKeyPress = event.parse()?;
        if let Some(seat) = self.seats.get(&event.deviceid) {
            seat.kb_event(InputEvent::Key(event.detail - 8, state));
        }
        Ok(())
    }

    async fn handle_input_hierarchy(
        self: &Rc<Self>,
        event: &Event,
    ) -> Result<(), XorgngBackendError> {
        let event: XiHierarchy = event.parse()?;
        for info in event.infos.iter() {
            if info.flags & INPUT_HIERARCHY_MASK_MASTER_ADDED != 0 {
                if let Err(e) = self.query_devices(info.deviceid).await {
                    log::error!("Could not query device {}: {:#}", info.deviceid, e);
                }
            } else if info.flags & INPUT_HIERARCHY_MASK_MASTER_REMOVED != 0 {
                self.mouse_seats.remove(&info.attachment);
                if let Some(seat) = self.seats.remove(&info.deviceid) {
                    seat.removed.set(true);
                    seat.kb_changed();
                    seat.mouse_changed();
                }
            }
        }
        Ok(())
    }

    fn handle_input_enter(&self, event: &Event) -> Result<(), XorgngBackendError> {
        let event: XiEnter = event.parse()?;
        if let (Some(win), Some(seat)) = (
            self.outputs.get(&event.event),
            self.mouse_seats.get(&event.deviceid),
        ) {
            seat.mouse_event(InputEvent::OutputPosition(
                win.id,
                Fixed::from_1616(event.event_x),
                Fixed::from_1616(event.event_y),
            ));
        }
        Ok(())
    }

    fn handle_input_motion(&self, event: &Event) -> Result<(), XorgngBackendError> {
        let event: XiMotion = event.parse()?;
        let (win, seat) = match (
            self.outputs.get(&event.event),
            self.mouse_seats.get(&event.deviceid),
        ) {
            (Some(a), Some(b)) => (a, b),
            _ => return Ok(()),
        };
        seat.mouse_event(InputEvent::OutputPosition(
            win.id,
            Fixed::from_1616(event.event_x),
            Fixed::from_1616(event.event_y),
        ));
        Ok(())
    }

    fn handle_destroy(&self, event: &Event) -> Result<(), XorgngBackendError> {
        self.state.el.stop();
        let event: DestroyNotify = event.parse()?;
        let output = match self.outputs.remove(&event.event) {
            Some(o) => o,
            _ => return Ok(()),
        };
        output.removed.set(true);
        output.changed();
        Ok(())
    }

    async fn handle_configure(&self, event: &Event) -> Result<(), XorgngBackendError> {
        let event: ConfigureNotify = event.parse()?;
        let output = match self.outputs.get(&event.event) {
            Some(o) => o,
            _ => return Ok(()),
        };
        let width = event.width as i32;
        let height = event.height as i32;
        let mut changed = false;
        changed |= output.width.replace(width) != width;
        changed |= output.height.replace(height) != height;
        if changed {
            let images = self.create_images(output.window, width, height).await?;
            for (new, old) in images.iter().zip(output.images.iter()) {
                let _ = self.c.call(&FreePixmap {
                    pixmap: old.pixmap.get(),
                });
                old.fb.set(new.fb.get());
                old.pixmap.set(new.pixmap.get());
            }
            output.changed();
        }
        Ok(())
    }
}

struct XorgOutput {
    id: OutputId,
    _backend: Rc<XorgngBackendData>,
    window: u32,
    removed: Cell<bool>,
    width: Cell<i32>,
    height: Cell<i32>,
    serial: NumCell<u32>,
    next_msc: Cell<u64>,
    next_image: NumCell<usize>,
    images: [XorgImage; 2],
    cb: CloneCell<Option<Rc<dyn Fn()>>>,
}

struct XorgImage {
    pixmap: Cell<u32>,
    fb: CloneCell<Rc<Framebuffer>>,
    idle: Cell<bool>,
    render_on_idle: Cell<bool>,
    last_serial: Cell<u32>,
}

impl XorgOutput {
    fn changed(&self) {
        if let Some(cb) = self.cb.get() {
            cb();
        }
    }
}

impl Output for XorgOutput {
    fn id(&self) -> OutputId {
        self.id
    }

    fn removed(&self) -> bool {
        self.removed.get()
    }

    fn width(&self) -> i32 {
        self.width.get()
    }

    fn height(&self) -> i32 {
        self.height.get()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.cb.set(Some(cb));
    }
}

struct XorgSeat {
    kb_id: InputDeviceId,
    mouse_id: InputDeviceId,
    backend: Rc<XorgngBackendData>,
    kb: u16,
    mouse: u16,
    removed: Cell<bool>,
    kb_cb: CloneCell<Option<Rc<dyn Fn()>>>,
    mouse_cb: CloneCell<Option<Rc<dyn Fn()>>>,
    kb_events: RefCell<VecDeque<InputEvent>>,
    mouse_events: RefCell<VecDeque<InputEvent>>,
    button_map: CopyHashMap<u32, u32>,
}

struct XorgSeatKeyboard(Rc<XorgSeat>);

struct XorgSeatMouse(Rc<XorgSeat>);

impl XorgSeat {
    fn kb_changed(&self) {
        if let Some(cb) = self.kb_cb.get() {
            cb();
        }
    }

    fn mouse_changed(&self) {
        if let Some(cb) = self.mouse_cb.get() {
            cb();
        }
    }

    fn mouse_event(&self, event: InputEvent) {
        self.mouse_events.borrow_mut().push_back(event);
        self.mouse_changed();
    }

    fn kb_event(&self, event: InputEvent) {
        self.kb_events.borrow_mut().push_back(event);
        self.kb_changed();
    }

    async fn update_button_map(&self) {
        self.button_map.clear();
        let gdbm = XiGetDeviceButtonMapping {
            device_id: self.mouse as _,
        };
        let reply = match self.backend.c.call(&gdbm).await {
            Ok(r) => r,
            Err(e) => {
                log::error!(
                    "Could not get Xinput button map of device {}: {}",
                    self.mouse,
                    ErrorFmt(e),
                );
                return;
            }
        };
        for (i, map) in reply.get().map.iter().copied().enumerate().rev() {
            self.button_map.set(map as u32, i as u32 + 1);
        }
    }
}

impl InputDevice for XorgSeatKeyboard {
    fn id(&self) -> InputDeviceId {
        self.0.kb_id
    }

    fn removed(&self) -> bool {
        self.0.removed.get()
    }

    fn event(&self) -> Option<InputEvent> {
        self.0.kb_events.borrow_mut().pop_front()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.0.kb_cb.set(Some(cb));
    }

    fn grab(&self, grab: bool) {
        self.0.backend.grab_requests.push((self.0.clone(), grab));
    }
}

impl InputDevice for XorgSeatMouse {
    fn id(&self) -> InputDeviceId {
        self.0.mouse_id
    }

    fn removed(&self) -> bool {
        self.0.removed.get()
    }

    fn event(&self) -> Option<InputEvent> {
        self.0.mouse_events.borrow_mut().pop_front()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.0.mouse_cb.set(Some(cb));
    }

    fn grab(&self, _grab: bool) {
        log::error!("Cannot grab xorg mouse");
    }
}
