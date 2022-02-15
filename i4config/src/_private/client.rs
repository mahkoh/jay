use crate::_private::ipc::{ClientMessage, InitMessage, Response, ServerMessage};
use crate::_private::{bincode_ops, logging, Config, ConfigEntry, ConfigEntryGen, VERSION};
use crate::keyboard::keymap::Keymap;
use crate::{Axis, Command, Direction, InputDevice, LogLevel, ModifiedKeySym, Seat};
use std::cell::{Cell, RefCell};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::{ptr, slice};

pub(crate) struct Client {
    configure: extern "C" fn(),
    srv_data: *const u8,
    srv_unref: unsafe extern "C" fn(data: *const u8),
    srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
    key_handlers: RefCell<HashMap<(Seat, ModifiedKeySym), Rc<dyn Fn()>>>,
    response: RefCell<Vec<Response>>,
    on_new_seat: RefCell<Option<Rc<dyn Fn(Seat)>>>,
    on_new_input_device: RefCell<Option<Rc<dyn Fn(InputDevice)>>>,
    bufs: RefCell<Vec<Vec<u8>>>,
}

impl Drop for Client {
    fn drop(&mut self) {
        unsafe {
            (self.srv_unref)(self.srv_data);
        }
    }
}

thread_local! {
    pub(crate) static CLIENT: std::cell::Cell<*const Client> = const { std::cell::Cell::new(ptr::null()) };
}

unsafe fn with_client<T, F: FnOnce(&Client) -> T>(data: *const u8, f: F) -> T {
    struct Reset<'a> {
        cell: &'a Cell<*const Client>,
        val: *const Client,
    }
    impl Drop for Reset<'_> {
        fn drop(&mut self) {
            self.cell.set(self.val);
        }
    }
    CLIENT.with(|cell| unsafe {
        let client = data as *const Client;
        Rc::increment_strong_count(client);
        let client = Rc::from_raw(client);
        let old = cell.replace(client.deref());
        let _reset = Reset { cell, val: old };
        f(&client)
    })
}

impl<T: Config> ConfigEntryGen<T> {
    pub const ENTRY: ConfigEntry = ConfigEntry {
        version: VERSION,
        init: Self::init,
        unref,
        handle_msg,
    };

    pub unsafe extern "C" fn init(
        srv_data: *const u8,
        srv_unref: unsafe extern "C" fn(data: *const u8),
        srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
        init_data: *const u8,
        size: usize,
    ) -> *const u8 {
        logging::init();
        init(
            srv_data,
            srv_unref,
            srv_handler,
            init_data,
            size,
            T::configure,
        )
    }
}

pub unsafe extern "C" fn init(
    srv_data: *const u8,
    srv_unref: unsafe extern "C" fn(data: *const u8),
    srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
    init: *const u8,
    size: usize,
    f: extern "C" fn(),
) -> *const u8 {
    let client = Rc::new(Client {
        configure: f,
        srv_data,
        srv_unref,
        srv_handler,
        key_handlers: Default::default(),
        response: Default::default(),
        on_new_seat: Default::default(),
        on_new_input_device: Default::default(),
        bufs: Default::default(),
    });
    let init = slice::from_raw_parts(init, size);
    client.handle_init_msg(init);
    Rc::into_raw(client) as *const u8
}

pub unsafe extern "C" fn unref(data: *const u8) {
    let client = data as *const Client;
    drop(Rc::from_raw(client));
}

pub unsafe extern "C" fn handle_msg(data: *const u8, msg: *const u8, size: usize) {
    with_client(data, |client| {
        let msg = slice::from_raw_parts(msg, size);
        client.handle_msg(msg);
    });
}

impl Client {
    fn send(&self, msg: &ClientMessage) {
        let mut buf = self.bufs.borrow_mut().pop().unwrap_or_default();
        buf.clear();
        bincode::encode_into_std_write(msg, &mut buf, bincode_ops()).unwrap();
        unsafe {
            (self.srv_handler)(self.srv_data, buf.as_ptr(), buf.len());
        }
        self.bufs.borrow_mut().push(buf);
    }

    pub fn spawn(&self, command: &Command) {
        let env = command.env.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        self.send(&ClientMessage::Run {
            prog: &command.prog,
            args: command.args.clone(),
            env,
        });
    }

    pub fn focus(&self, seat: Seat, direction: Direction) {
        self.send(&ClientMessage::Focus { seat, direction });
    }

    pub fn move_(&self, seat: Seat, direction: Direction) {
        self.send(&ClientMessage::Move { seat, direction });
    }

    pub fn unbind<T: Into<ModifiedKeySym>>(&self, seat: Seat, mod_sym: T) {
        let mod_sym = mod_sym.into();
        let deregister = self
            .key_handlers
            .borrow_mut()
            .remove(&(seat, mod_sym))
            .is_some();
        if deregister {
            self.send(&ClientMessage::RemoveShortcut {
                seat,
                mods: mod_sym.mods,
                sym: mod_sym.sym,
            })
        }
    }

    fn with_response<F: FnOnce()>(&self, f: F) -> Response {
        f();
        self.response.borrow_mut().pop().unwrap_or(Response::None)
    }

    pub fn seats(&self) -> Vec<Seat> {
        let response = self.with_response(|| self.send(&ClientMessage::GetSeats));
        match response {
            Response::GetSeats { seats } => seats,
            _ => {
                log::error!("Server did not send a response to a get_seats request");
                vec![]
            }
        }
    }

    pub fn split(&self, seat: Seat) -> Axis {
        let res = self.with_response(|| self.send(&ClientMessage::GetSplit { seat }));
        match res {
            Response::GetSplit { axis } => axis,
            _ => {
                log::error!("Server did not send a response to a get_split request");
                Axis::Horizontal
            }
        }
    }

    pub fn set_split(&self, seat: Seat, axis: Axis) {
        self.send(&ClientMessage::SetSplit { seat, axis });
    }

    pub fn create_seat(&self, name: &str) -> Seat {
        let response = self.with_response(|| self.send(&ClientMessage::CreateSeat { name }));
        match response {
            Response::CreateSeat { seat } => seat,
            _ => {
                log::error!("Server did not send a response to a create_seat request");
                Seat(0)
            }
        }
    }

    pub fn get_input_devices(&self) -> Vec<InputDevice> {
        let res = self.with_response(|| self.send(&ClientMessage::GetInputDevices));
        match res {
            Response::GetInputDevices { devices } => devices,
            _ => {
                log::error!("Server did not send a response to a get_input_devices request");
                vec![]
            }
        }
    }

    pub fn on_new_seat<F: Fn(Seat) + 'static>(&self, f: F) {
        *self.on_new_seat.borrow_mut() = Some(Rc::new(f));
    }

    pub fn on_new_input_device<F: Fn(InputDevice) + 'static>(&self, f: F) {
        *self.on_new_input_device.borrow_mut() = Some(Rc::new(f));
    }

    pub fn set_seat(&self, device: InputDevice, seat: Seat) {
        self.send(&ClientMessage::SetSeat { device, seat })
    }

    pub fn seat_set_keymap(&self, seat: Seat, keymap: Keymap) {
        self.send(&ClientMessage::SeatSetKeymap { seat, keymap })
    }

    pub fn seat_set_repeat_rate(&self, seat: Seat, rate: i32, delay: i32) {
        self.send(&ClientMessage::SeatSetRepeatRate { seat, rate, delay })
    }

    pub fn seat_get_repeat_rate(&self, seat: Seat) -> (i32, i32) {
        let res = self.with_response(|| self.send(&ClientMessage::SeatGetRepeatRate { seat }));
        match res {
            Response::GetRepeatRate { rate, delay } => (rate, delay),
            _ => {
                log::error!("Server did not send a response to a get_repeat_rate request");
                (25, 250)
            }
        }
    }

    pub fn parse_keymap(&self, keymap: &str) -> Keymap {
        let res = self.with_response(|| self.send(&ClientMessage::ParseKeymap { keymap }));
        match res {
            Response::ParseKeymap { keymap } => keymap,
            _ => {
                log::error!("Server did not send a response to a parse_keymap request");
                Keymap(0)
            }
        }
    }

    pub fn bind<T: Into<ModifiedKeySym>, F: Fn() + 'static>(&self, seat: Seat, mod_sym: T, f: F) {
        let mod_sym = mod_sym.into();
        let register = {
            let mut kh = self.key_handlers.borrow_mut();
            let f = Rc::new(f);
            match kh.entry((seat, mod_sym)) {
                Entry::Occupied(mut o) => {
                    *o.get_mut() = f;
                    false
                }
                Entry::Vacant(v) => {
                    v.insert(f);
                    true
                }
            }
        };
        if register {
            self.send(&ClientMessage::AddShortcut {
                seat,
                mods: mod_sym.mods,
                sym: mod_sym.sym,
            });
        }
    }

    pub fn log(&self, level: LogLevel, msg: &str, file: Option<&str>, line: Option<u32>) {
        self.send(&ClientMessage::Log {
            level,
            msg,
            file,
            line,
        })
    }

    fn handle_msg(&self, msg: &[u8]) {
        let res = bincode::decode_from_slice::<ServerMessage, _>(msg, bincode_ops());
        let (msg, _) = match res {
            Ok(msg) => msg,
            Err(e) => {
                let msg = format!("could not deserialize message: {}", e);
                self.log(LogLevel::Error, &msg, None, None);
                return;
            }
        };
        match msg {
            ServerMessage::Configure => {
                (self.configure)();
            }
            ServerMessage::Response { response } => {
                self.response.borrow_mut().push(response);
            }
            ServerMessage::InvokeShortcut { seat, mods, sym } => {
                let ms = ModifiedKeySym { mods, sym };
                let handler = self.key_handlers.borrow_mut().get(&(seat, ms)).cloned();
                if let Some(handler) = handler {
                    handler();
                }
            }
            ServerMessage::NewInputDevice { device } => {
                let handler = self.on_new_input_device.borrow_mut().clone();
                if let Some(handler) = handler {
                    handler(device);
                }
            }
            ServerMessage::DelInputDevice { .. } => {}
        }
    }

    fn handle_init_msg(&self, msg: &[u8]) {
        let (init, _) = match bincode::decode_from_slice::<InitMessage, _>(msg, bincode_ops()) {
            Ok(m) => m,
            Err(e) => {
                let msg = format!("could not deserialize message: {}", e);
                self.log(LogLevel::Error, &msg, None, None);
                return;
            }
        };
        match init {
            InitMessage::V1(_) => {}
        }
    }
}
