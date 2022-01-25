use crate::backend::{
    BackendEvent, KeyState, Output, OutputId, ScrollAxis, Seat, SeatEvent, SeatId,
};
use crate::event_loop::{EventLoopDispatcher, EventLoopId};
use crate::fixed::Fixed;
use crate::pixman::{Image, PixmanError};
use crate::render::pixman::PixmanRenderer;
use crate::render::Renderer;
use crate::servermem::{ServerMem, ServerMemError};
use crate::tree::Node;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::ptr_ext::PtrExt;
use crate::wheel::{WheelDispatcher, WheelId};
use crate::{pixman, EventLoopError, State, WheelError};
use isnt::std_1::primitive::IsntConstPtrExt;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::error::Error;
use std::ops::Deref;
use std::ptr;
use std::rc::Rc;
use thiserror::Error;
use uapi::c;
use xcb_dl::{ffi, Xcb, XcbDri3, XcbPresent, XcbShm, XcbXinput, XcbXkb};
use xcb_dl_util::error::{XcbError, XcbErrorParser};
use xcb_dl_util::xcb_box::XcbBox;

#[derive(Debug, Error)]
pub enum XorgBackendError {
    #[error("The xcb connection is in an error state")]
    ErrorEvent,
    #[error("Could not select input events")]
    CannotSelectInputEvents(#[source] XcbError),
    #[error("Could not select present events")]
    CannotSelectPresentEvents(#[source] XcbError),
    #[error("libloading returned an error")]
    Libloading(#[from] libloading::Error),
    #[error("xcb returned an error")]
    XcbError(#[from] XcbError),
    #[error("The event loop caused an error")]
    EventLoopError(#[source] Box<EventLoopError>),
    #[error("The timer wheel caused an error")]
    WheelError(#[source] Box<WheelError>),
    #[error("Could not allocate and map image memory")]
    ServerMemError(#[source] Box<ServerMemError>),
    #[error("Pixman returned an error")]
    PixmanError(#[source] Box<PixmanError>),
    #[error("dupfd failed")]
    DupfdFailed(#[source] std::io::Error),
    #[error("Could not create a window")]
    CreateWindow(#[source] XcbError),
    #[error("Could not set WM_CLASS")]
    WmClass(#[source] XcbError),
    #[error("Could not select window events")]
    WindowEvents(#[source] XcbError),
    #[error("Could not map a window")]
    MapWindow(#[source] XcbError),
    #[error("Could not query device")]
    QueryDevice(#[source] XcbError),
}
efrom!(XorgBackendError, EventLoopError, EventLoopError);
efrom!(XorgBackendError, ServerMemError, ServerMemError);
efrom!(XorgBackendError, PixmanError, PixmanError);
efrom!(XorgBackendError, WheelError, WheelError);

struct XcbCon {
    xcb: Box<Xcb>,
    shm: Box<XcbShm>,
    input: Box<XcbXinput>,
    dri: Box<XcbDri3>,
    present: Box<XcbPresent>,
    input_opcode: u8,
    present_opcode: u8,
    xkb: Box<XcbXkb>,
    c: *mut ffi::xcb_connection_t,
    errors: XcbErrorParser,
}

impl XcbCon {
    fn new() -> Result<Self, XorgBackendError> {
        unsafe {
            let xcb = Box::new(Xcb::load_loose()?);
            let shm = Box::new(XcbShm::load_loose()?);
            let input = Box::new(XcbXinput::load_loose()?);
            let xkb = Box::new(XcbXkb::load_loose()?);
            let dri = Box::new(XcbDri3::load_loose()?);

            let c = xcb.xcb_connect(ptr::null(), ptr::null_mut());
            let errors = XcbErrorParser::new(&xcb, c);

            let mut con = Self {
                xcb,
                shm,
                input,
                dri,
                input_opcode: 0,
                present_opcode: 0,
                xkb,
                c,
                errors,
            };

            con.errors.check_connection(&con.xcb)?;

            let mut err = ptr::null_mut();

            let res =
                con.shm
                    .xcb_shm_query_version_reply(c, con.shm.xcb_shm_query_version(c), &mut err);
            con.errors.check(&con.xcb, res, err)?;

            let res = con.input.xcb_input_xi_query_version_reply(
                c,
                con.input.xcb_input_xi_query_version(c, 2, 2),
                &mut err,
            );
            con.errors.check(&con.xcb, res, err)?;

            let input_ex = con
                .xcb
                .xcb_get_extension_data(con.c, con.input.xcb_input_id());
            assert!(input_ex.is_not_null());
            con.input_opcode = input_ex.deref().major_opcode;

            let res = con.dri.xcb_dri3_query_version_reply(
                c,
                con.dri.xcb_dri3_query_version(c, 1, 0),
                &mut err,
            );
            con.errors.check(&con.xcb, res, err)?;

            let res = con.present.xcb_present_query_version_reply(
                c,
                con.present.xcb_present_query_version(c, 1, 0),
                &mut err,
            );
            con.errors.check(&con.xcb, res, err)?;

            let present_ex = con
                .xcb
                .xcb_get_extension_data(con.c, con.present.xcb_present_id());
            assert!(present_ex.is_not_null());
            con.present_opcode = present_ex.deref().major_opcode;

            let res = con.xkb.xcb_xkb_use_extension_reply(
                c,
                con.xkb.xcb_xkb_use_extension(c, 1, 0),
                &mut err,
            );
            con.errors.check(&con.xcb, res, err)?;

            Ok(con)
        }
    }

    fn check_cookie(&self, cookie: ffi::xcb_void_cookie_t) -> Result<(), XcbError> {
        unsafe { self.errors.check_cookie(&self.xcb, cookie) }
    }

    fn check<T>(
        &self,
        reply: *mut T,
        err: *mut ffi::xcb_generic_error_t,
    ) -> Result<XcbBox<T>, XcbError> {
        unsafe { self.errors.check(&self.xcb, reply, err) }
    }

    fn screen(&self) -> &ffi::xcb_screen_t {
        unsafe {
            self.xcb
                .xcb_setup_roots_iterator(self.xcb.xcb_get_setup(self.c))
                .data
                .deref()
        }
    }
}

impl Drop for XcbCon {
    fn drop(&mut self) {
        unsafe {
            self.xcb.xcb_disconnect(self.c);
        }
    }
}

pub struct XorgBackend {
    id: EventLoopId,
    wheel_id: WheelId,
    state: Rc<State>,
    con: XcbCon,
    outputs: CopyHashMap<ffi::xcb_window_t, Rc<XorgOutput>>,
    seats: CopyHashMap<ffi::xcb_input_device_id_t, Rc<XorgSeat>>,
    mouse_seats: CopyHashMap<ffi::xcb_input_device_id_t, Rc<XorgSeat>>,
}

impl XorgBackend {
    pub fn new(state: &Rc<State>) -> Result<Rc<Self>, XorgBackendError> {
        unsafe {
            let con = XcbCon::new()?;

            let fd = con.xcb.xcb_get_file_descriptor(con.c);

            let wheel_id = state.wheel.id();

            let slf = Rc::new(Self {
                id: state.el.id(),
                wheel_id,
                state: state.clone(),
                con,
                outputs: Default::default(),
                seats: Default::default(),
                mouse_seats: Default::default(),
            });

            {
                let cookie = xcb_dl_util::input::select_events_checked(
                    &slf.con.input,
                    slf.con.c,
                    slf.con.screen().root,
                    ffi::XCB_INPUT_DEVICE_ALL as _,
                    [ffi::XCB_INPUT_XI_EVENT_MASK_HIERARCHY],
                );
                if let Err(e) = slf.con.check_cookie(cookie) {
                    return Err(XorgBackendError::CannotSelectInputEvents(e));
                }
            }

            state.wheel.periodic(wheel_id, 16_667, slf.clone())?;
            // state.wheel.periodic(wheel_id, 1000_000, slf.clone())?;
            state.el.insert(slf.id, Some(fd), c::EPOLLIN, slf.clone())?;

            slf.add_output()?;
            slf.query_devices(ffi::XCB_INPUT_DEVICE_ALL_MASTER as _)?;
            slf.handle_events()?;

            Ok(slf)
        }
    }

    fn add_output(self: &Rc<Self>) -> Result<(), XorgBackendError> {
        unsafe {
            let con = &self.con;
            let screen = con
                .xcb
                .xcb_setup_roots_iterator(con.xcb.xcb_get_setup(con.c))
                .data
                .deref();
            let window_id = con.xcb.xcb_generate_id(con.c);
            {
                let cookie = con.xcb.xcb_create_window_checked(
                    con.c,
                    0,
                    window_id,
                    screen.root,
                    0,
                    0,
                    800,
                    600,
                    0,
                    ffi::XCB_WINDOW_CLASS_INPUT_OUTPUT as _,
                    0,
                    0,
                    ptr::null(),
                );
                if let Err(e) = con.check_cookie(cookie) {
                    return Err(XorgBackendError::CreateWindow(e));
                }
            }
            let output = Rc::new(XorgOutput {
                id: self.state.output_ids.next(),
                backend: self.clone(),
                window: window_id,
                removed: Cell::new(false),
                width: Cell::new(0),
                height: Cell::new(0),
                image: RefCell::new(None),
                cb: CloneCell::new(None),
            });
            {
                let class = "i4\0i4\0";
                let cookie = con.xcb.xcb_change_property_checked(
                    con.c,
                    ffi::XCB_PROP_MODE_REPLACE as _,
                    window_id,
                    ffi::XCB_ATOM_WM_CLASS,
                    ffi::XCB_ATOM_STRING,
                    8,
                    class.len() as _,
                    class.as_ptr() as _,
                );
                if let Err(e) = con.check_cookie(cookie) {
                    return Err(XorgBackendError::WmClass(e));
                }
            }
            {
                let event_mask = ffi::XCB_EVENT_MASK_EXPOSURE
                    | ffi::XCB_EVENT_MASK_STRUCTURE_NOTIFY
                    | ffi::XCB_EVENT_MASK_VISIBILITY_CHANGE;
                let cookie = con.xcb.xcb_change_window_attributes_checked(
                    con.c,
                    window_id,
                    ffi::XCB_CW_EVENT_MASK,
                    &event_mask as *const _ as _,
                );
                if let Err(e) = con.check_cookie(cookie) {
                    return Err(XorgBackendError::WindowEvents(e));
                }
            }
            {
                let cookie = con.xcb.xcb_map_window_checked(con.c, window_id);
                if let Err(e) = con.check_cookie(cookie) {
                    return Err(XorgBackendError::MapWindow(e));
                }
            }
            {
                let mask = 0
                    | ffi::XCB_INPUT_XI_EVENT_MASK_MOTION
                    | ffi::XCB_INPUT_XI_EVENT_MASK_BUTTON_PRESS
                    | ffi::XCB_INPUT_XI_EVENT_MASK_BUTTON_RELEASE
                    | ffi::XCB_INPUT_XI_EVENT_MASK_KEY_PRESS
                    | ffi::XCB_INPUT_XI_EVENT_MASK_KEY_RELEASE
                    | ffi::XCB_INPUT_XI_EVENT_MASK_ENTER
                    | ffi::XCB_INPUT_XI_EVENT_MASK_LEAVE
                    | ffi::XCB_INPUT_XI_EVENT_MASK_FOCUS_IN
                    | ffi::XCB_INPUT_XI_EVENT_MASK_FOCUS_OUT
                    | ffi::XCB_INPUT_XI_EVENT_MASK_TOUCH_BEGIN
                    | ffi::XCB_INPUT_XI_EVENT_MASK_TOUCH_UPDATE
                    | ffi::XCB_INPUT_XI_EVENT_MASK_TOUCH_END;
                let cookie = xcb_dl_util::input::select_events_checked(
                    &con.input,
                    con.c,
                    window_id,
                    ffi::XCB_INPUT_DEVICE_ALL_MASTER as _,
                    [mask],
                );
                if let Err(e) = con.check_cookie(cookie) {
                    return Err(XorgBackendError::CannotSelectInputEvents(e));
                }
            }
            {
                let mask = 0
                    | ffi::XCB_PRESENT_EVENT_MASK_IDLE_NOTIFY
                    | ffi::XCB_PRESENT_EVENT_MASK_COMPLETE_NOTIFY;
                let cookie = con.present.xcb_present_select_input_checked(
                    con.c,
                    con.xcb.xcb_generate_id(con.c),
                    window_id,
                    mask,
                );
                if let Err(e) = con.check_cookie(cookie) {
                    return Err(XorgBackendError::CannotSelectPresentEvents(e));
                }
            }
            self.outputs.set(window_id, output.clone());
            self.state
                .backend_events
                .push(BackendEvent::NewOutput(output.clone()));
        }
        Ok(())
    }

    fn query_devices(
        self: &Rc<Self>,
        device_id: ffi::xcb_input_device_id_t,
    ) -> Result<(), XorgBackendError> {
        unsafe {
            let con = &self.con;
            let mut err = ptr::null_mut();
            let reply = con.input.xcb_input_xi_query_device_reply(
                con.c,
                con.input.xcb_input_xi_query_device(con.c, device_id),
                &mut err,
            );
            let reply = match con.check(reply, err) {
                Ok(i) => i,
                Err(e) => return Err(XorgBackendError::QueryDevice(e)),
            };
            let mut iter = con.input.xcb_input_xi_query_device_infos_iterator(&*reply);
            while iter.rem > 0 {
                self.handle_input_device(iter.data.deref());
                con.input.xcb_input_xi_device_info_next(&mut iter);
            }
        }
        Ok(())
    }

    fn handle_input_device(self: &Rc<Self>, info: &ffi::xcb_input_xi_device_info_t) {
        if info.type_ != ffi::XCB_INPUT_DEVICE_TYPE_MASTER_KEYBOARD as _ {
            return;
        }
        let con = &self.con;
        self.mouse_seats.remove(&info.attachment);
        if let Some(kb) = self.seats.remove(&info.deviceid) {
            kb.removed.set(true);
            kb.changed();
        }
        unsafe {
            let mut err = ptr::null_mut();
            let cookie = con.xkb.xcb_xkb_per_client_flags(
                con.c,
                info.deviceid,
                ffi::XCB_XKB_PER_CLIENT_FLAG_DETECTABLE_AUTO_REPEAT,
                ffi::XCB_XKB_PER_CLIENT_FLAG_DETECTABLE_AUTO_REPEAT,
                0,
                0,
                0,
            );
            let reply = con
                .xkb
                .xcb_xkb_per_client_flags_reply(con.c, cookie, &mut err);
            if let Err(e) = con.check(reply, err) {
                log::warn!(
                    "Could not make auto repeat detectable for keyboard {}: {:#}",
                    info.deviceid,
                    e
                );
            }
            let seat = Rc::new(XorgSeat {
                id: self.state.seat_ids.next(),
                backend: self.clone(),
                _kb: info.deviceid,
                mouse: info.attachment,
                removed: Cell::new(false),
                cb: CloneCell::new(None),
                events: RefCell::new(Default::default()),
                button_map: Default::default(),
            });
            seat.update_button_map();
            self.seats.set(info.deviceid, seat.clone());
            self.mouse_seats.set(info.attachment, seat.clone());
            self.state
                .backend_events
                .push(BackendEvent::NewSeat(seat.clone()));
        }
    }

    fn handle_events(self: &Rc<Self>) -> Result<(), XorgBackendError> {
        unsafe {
            loop {
                let event = self.con.xcb.xcb_poll_for_event(self.con.c);
                if event.is_null() {
                    self.con.errors.check_connection(&self.con.xcb)?;
                    return Ok(());
                }
                let event = XcbBox::new(event);
                self.handle_event(&event)?;
            }
        }
    }

    fn handle_event(
        self: &Rc<Self>,
        event: &ffi::xcb_generic_event_t,
    ) -> Result<(), XorgBackendError> {
        let event_type = event.response_type & 0x7f;
        match event_type {
            ffi::XCB_CONFIGURE_NOTIFY => self.handle_configure(event)?,
            ffi::XCB_DESTROY_NOTIFY => self.handle_destroy(event)?,
            ffi::XCB_GE_GENERIC => self.handle_generic(event)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_generic(
        self: &Rc<Self>,
        event: &ffi::xcb_generic_event_t,
    ) -> Result<(), XorgBackendError> {
        let event = unsafe { (event as *const _ as *const ffi::xcb_ge_generic_event_t).deref() };
        if event.extension == self.con.input_opcode {
            self.handle_input_event(event)?;
        } else if event.extension == self.con.present_opcode {
            self.handle_present_event(event)?;
        }
        Ok(())
    }

    fn handle_present_event(
        self: &Rc<Self>,
        event: &ffi::xcb_ge_generic_event_t,
    ) -> Result<(), XorgBackendError> {
        match event.event_type {
            ffi::XCB_PRESENT_COMPLETE_NOTIFY => self.handle_present_complete(event)?,
            ffi::XCB_PRESENT_IDLE_NOTIFY => self.handle_present_idle(event)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_present_complete(
        self: &Rc<Self>,
        event: &ffi::xcb_ge_generic_event_t,
    ) -> Result<(), XorgBackendError> {
        let event = unsafe {
            (event as *const _ as *const ffi::xcb_present_complete_notify_event_t).deref()
        };
        let output = match self.outputs.get(&event.window) {
            Some(o) => o,
            _ => return Ok(()),
        };
        output.next_msc.set(event.msc + 1);
        Ok(())
    }

    fn handle_input_event(
        self: &Rc<Self>,
        event: &ffi::xcb_ge_generic_event_t,
    ) -> Result<(), XorgBackendError> {
        match event.event_type {
            ffi::XCB_INPUT_MOTION => self.handle_input_motion(event)?,
            ffi::XCB_INPUT_ENTER => self.handle_input_enter(event)?,
            ffi::XCB_INPUT_BUTTON_PRESS => {
                self.handle_input_button_press(event, KeyState::Pressed)?
            }
            ffi::XCB_INPUT_BUTTON_RELEASE => {
                self.handle_input_button_press(event, KeyState::Released)?
            }
            ffi::XCB_INPUT_KEY_PRESS => self.handle_input_key_press(event, KeyState::Pressed)?,
            ffi::XCB_INPUT_KEY_RELEASE => self.handle_input_key_press(event, KeyState::Released)?,
            ffi::XCB_INPUT_HIERARCHY => self.handle_input_hierarchy(event)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_input_button_press(
        self: &Rc<Self>,
        event: &ffi::xcb_ge_generic_event_t,
        state: KeyState,
    ) -> Result<(), XorgBackendError> {
        let event =
            unsafe { (event as *const _ as *const ffi::xcb_input_button_press_event_t).deref() };
        if let Some(seat) = self.mouse_seats.get(&event.deviceid) {
            let button = seat.button_map.get(&event.detail).unwrap_or(event.detail);
            if matches!(button, 4..=7) {
                if state == KeyState::Pressed {
                    let (axis, val) = match button {
                        4 => (ScrollAxis::Vertical, -15),
                        5 => (ScrollAxis::Vertical, 15),
                        6 => (ScrollAxis::Horizontal, -15),
                        7 => (ScrollAxis::Horizontal, 15),
                        _ => unreachable!(),
                    };
                    seat.event(SeatEvent::Scroll(val, axis));
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
                seat.event(SeatEvent::Button(button, state));
            }
        }
        Ok(())
    }

    fn handle_input_key_press(
        self: &Rc<Self>,
        event: &ffi::xcb_ge_generic_event_t,
        state: KeyState,
    ) -> Result<(), XorgBackendError> {
        let event =
            unsafe { (event as *const _ as *const ffi::xcb_input_key_press_event_t).deref() };
        if let Some(seat) = self.seats.get(&event.deviceid) {
            seat.event(SeatEvent::Key(event.detail - 8, state));
        }
        Ok(())
    }

    fn handle_input_hierarchy(
        self: &Rc<Self>,
        event: &ffi::xcb_ge_generic_event_t,
    ) -> Result<(), XorgBackendError> {
        let event =
            unsafe { (event as *const _ as *const ffi::xcb_input_hierarchy_event_t).deref() };
        let infos = unsafe {
            std::slice::from_raw_parts(
                self.con.input.xcb_input_hierarchy_infos(event),
                event.num_infos as _,
            )
        };
        for info in infos {
            if info.flags & ffi::XCB_INPUT_HIERARCHY_MASK_MASTER_ADDED != 0 {
                if let Err(e) = self.query_devices(info.deviceid) {
                    log::error!("Could not query device {}: {:#}", info.deviceid, e);
                }
            } else if info.flags & ffi::XCB_INPUT_HIERARCHY_MASK_MASTER_REMOVED != 0 {
                self.mouse_seats.remove(&info.attachment);
                if let Some(seat) = self.seats.remove(&info.deviceid) {
                    seat.removed.set(true);
                    seat.changed();
                }
            }
        }
        Ok(())
    }

    fn handle_input_enter(
        &self,
        event: &ffi::xcb_ge_generic_event_t,
    ) -> Result<(), XorgBackendError> {
        let event = unsafe { (event as *const _ as *const ffi::xcb_input_enter_event_t).deref() };
        if let (Some(win), Some(seat)) = (
            self.outputs.get(&event.event),
            self.mouse_seats.get(&event.deviceid),
        ) {
            seat.event(SeatEvent::OutputPosition(
                win.id,
                Fixed::from_1616(event.event_x),
                Fixed::from_1616(event.event_y),
            ));
        }
        Ok(())
    }

    fn handle_input_motion(
        &self,
        event: &ffi::xcb_ge_generic_event_t,
    ) -> Result<(), XorgBackendError> {
        let event = unsafe { (event as *const _ as *const ffi::xcb_input_motion_event_t).deref() };
        let (win, seat) = match (self.outputs.get(&event.event), self.mouse_seats.get(&event.deviceid)) {
            (Some(a), Some(b)) => (a, b),
            _ => return Ok(()),
        };
        seat.event(SeatEvent::OutputPosition(
            win.id,
            Fixed::from_1616(event.event_x),
            Fixed::from_1616(event.event_y),
        ));
        Ok(())
    }

    fn handle_destroy(&self, event: &ffi::xcb_generic_event_t) -> Result<(), XorgBackendError> {
        let event =
            unsafe { (event as *const _ as *const ffi::xcb_destroy_notify_event_t).deref() };
        let output = match self.outputs.remove(&event.event) {
            Some(o) => o,
            _ => return Ok(()),
        };
        output.removed.set(true);
        *output.image.borrow_mut() = None;
        output.changed();
        Ok(())
    }

    fn handle_configure(&self, event: &ffi::xcb_generic_event_t) -> Result<(), XorgBackendError> {
        let event =
            unsafe { (event as *const _ as *const ffi::xcb_configure_notify_event_t).deref() };
        let output = match self.outputs.get(&event.event) {
            Some(o) => o,
            _ => return Ok(()),
        };
        let width = event.width as u32;
        let height = event.height as u32;
        let mut changed = false;
        changed |= output.width.replace(width as i32) != width as i32;
        changed |= output.height.replace(height as i32) != height as i32;
        if changed {
            let shm = Rc::new(ServerMem::new((width * height * 4) as usize)?);
            let fd = shm.fd();
            let image = Image::new(shm, pixman::X8R8G8B8, width, height, width * 4)?;
            *output.image.borrow_mut() = Some(image);
            unsafe {
                let fd = match uapi::fcntl_dupfd_cloexec(fd, 0) {
                    Ok(fd) => fd,
                    Err(e) => return Err(XorgBackendError::DupfdFailed(e.into())),
                };
                let shmseg = self.con.xcb.xcb_generate_id(self.con.c);
                let cookie =
                    self.con
                        .shm
                        .xcb_shm_attach_fd_checked(self.con.c, shmseg, fd.unwrap(), 0);
                self.con.check_cookie(cookie)?;
                let pixmap = self.con.xcb.xcb_generate_id(self.con.c);
                let cookie = self.con.shm.xcb_shm_create_pixmap(
                    self.con.c,
                    pixmap,
                    output.window,
                    width as _,
                    height as _,
                    24,
                    shmseg,
                    0,
                );
                self.con.check_cookie(cookie)?;
                let cookie = self.con.xcb.xcb_change_window_attributes_checked(
                    self.con.c,
                    output.window,
                    ffi::XCB_CW_BACK_PIXMAP,
                    &pixmap as *const _ as _,
                );
                self.con.check_cookie(cookie)?;
                self.con.xcb.xcb_free_pixmap(self.con.c, pixmap);
                self.con.shm.xcb_shm_detach(self.con.c, shmseg);
            }
            output.changed();
            self.render()?;
        }
        Ok(())
    }

    fn render(&self) -> Result<(), XorgBackendError> {
        let outputs = self.outputs.lock();
        for output in outputs.values() {
            let image = output.image.borrow();
            let image = match image.deref() {
                Some(i) => i,
                None => continue,
            };
            let _ = image.fill(0, 0, 0, 255);
            let node = match self.state.root.outputs.get(&output.id) {
                Some(n) => n,
                _ => continue,
            };
            let pos = node.position.get();
            let mut renderer = PixmanRenderer::new(image);
            renderer.render_output(&node);
            for floating in self.state.root.floaters.iter() {
                let fpos = floating.position.get();
                if floating.visible.get() && fpos.intersects(&pos) {
                    let (x, y) = pos.translate(fpos.x1(), fpos.x2());
                    floating.render(&mut renderer, x, y);
                }
            }
            unsafe {
                let cookie =
                    self.con
                        .xcb
                        .xcb_clear_area_checked(self.con.c, 0, output.window, 0, 0, 0, 0);
                self.con.check_cookie(cookie)?;
            }
        }
        Ok(())
    }
}

impl EventLoopDispatcher for XorgBackend {
    fn dispatch(self: Rc<Self>, events: i32) -> Result<(), Box<dyn Error + Send + Sync>> {
        if events & (c::EPOLLERR | c::EPOLLHUP) != 0 {
            return Err(Box::new(XorgBackendError::ErrorEvent));
        }
        self.handle_events()?;
        Ok(())
    }
}

impl WheelDispatcher for XorgBackend {
    fn dispatch(self: Rc<Self>) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.render()?;
        self.handle_events()?;
        Ok(())
    }
}

impl Drop for XorgBackend {
    fn drop(&mut self) {
        let _ = self.state.el.remove(self.id);
        let _ = self.state.wheel.remove(self.wheel_id);
    }
}

struct XorgOutput {
    id: OutputId,
    backend: Rc<XorgBackend>,
    window: ffi::xcb_window_t,
    removed: Cell<bool>,
    width: Cell<i32>,
    height: Cell<i32>,
    next_msc: Cell<u64>,
    next_image: Cell<usize>,
    // images: [XorgImage; 2],
    image: RefCell<Option<Image<Rc<ServerMem>>>>,
    cb: CloneCell<Option<Rc<dyn Fn()>>>,
}

struct XorgImage {

}

impl Drop for XorgOutput {
    fn drop(&mut self) {
        unsafe {
            let con = &self.backend.con;
            con.xcb.xcb_destroy_window(con.c, self.window);
        }
    }
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
    id: SeatId,
    backend: Rc<XorgBackend>,
    _kb: ffi::xcb_input_device_id_t,
    mouse: ffi::xcb_input_device_id_t,
    removed: Cell<bool>,
    cb: CloneCell<Option<Rc<dyn Fn()>>>,
    events: RefCell<VecDeque<SeatEvent>>,
    button_map: CopyHashMap<u32, u32>,
}

impl XorgSeat {
    fn changed(&self) {
        if let Some(cb) = self.cb.get() {
            cb();
        }
    }

    fn event(&self, event: SeatEvent) {
        self.events.borrow_mut().push_back(event);
        self.changed();
    }

    fn update_button_map(&self) {
        self.button_map.clear();
        unsafe {
            let con = &self.backend.con;
            let mut err = ptr::null_mut();
            let reply = con.input.xcb_input_get_device_button_mapping_reply(
                con.c,
                con.input
                    .xcb_input_get_device_button_mapping(con.c, self.mouse as _),
                &mut err,
            );
            let reply = match con.check(reply, err) {
                Ok(r) => r,
                Err(e) => {
                    log::error!(
                        "Could not get Xinput button map of device {}: {:#}",
                        self.mouse,
                        e
                    );
                    return;
                }
            };
            let map = std::slice::from_raw_parts(
                con.input.xcb_input_get_device_button_mapping_map(&*reply),
                reply.map_size as _,
            );
            for (i, map) in map.iter().copied().enumerate().rev() {
                self.button_map.set(map as u32, i as u32 + 1);
            }
        }
    }
}

impl Seat for XorgSeat {
    fn id(&self) -> SeatId {
        self.id
    }

    fn removed(&self) -> bool {
        self.removed.get()
    }

    fn event(&self) -> Option<SeatEvent> {
        self.events.borrow_mut().pop_front()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.cb.set(Some(cb));
    }
}
