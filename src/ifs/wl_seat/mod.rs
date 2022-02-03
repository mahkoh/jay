mod handling;
mod types;
pub mod wl_keyboard;
pub mod wl_pointer;
pub mod wl_touch;

use crate::backend::{Seat, SeatId};
use crate::client::{Client, ClientId, DynEventFormatter};
use crate::cursor::{Cursor, KnownCursor};
use crate::fixed::Fixed;
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_data_device::{WlDataDevice, WlDataDeviceId};
use crate::ifs::wl_seat::wl_keyboard::{WlKeyboard, WlKeyboardId, REPEAT_INFO_SINCE};
use crate::ifs::wl_seat::wl_pointer::{WlPointer, WlPointerId};
use crate::ifs::wl_seat::wl_touch::WlTouch;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::object::{Interface, Object, ObjectId};
use crate::tree::{FloatNode, FoundNode, Node};
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::LinkedList;
use crate::xkbcommon::{XkbContext, XkbState};
use crate::{NumCell, State};
use ahash::{AHashMap, AHashSet};
use bstr::ByteSlice;
pub use handling::NodeSeatState;
use std::cell::{Cell, RefCell};
use std::collections::hash_map::Entry;
use std::io::Write;
use std::rc::Rc;
pub use types::*;
use uapi::{c, OwnedFd};

id!(WlSeatId);

const GET_POINTER: u32 = 0;
const GET_KEYBOARD: u32 = 1;
const GET_TOUCH: u32 = 2;
const RELEASE: u32 = 3;

const CAPABILITIES: u32 = 0;
const NAME: u32 = 1;

const POINTER: u32 = 1;
const KEYBOARD: u32 = 2;
#[allow(dead_code)]
const TOUCH: u32 = 4;

#[allow(dead_code)]
const MISSING_CAPABILITY: u32 = 0;

pub const BTN_LEFT: u32 = 0x110;

pub const SEAT_NAME_SINCE: u32 = 2;

pub struct PointerGrab {
    seat: Rc<WlSeatGlobal>,
}

pub struct PointerGrabber {
    node: Rc<dyn Node>,
}

impl Drop for PointerGrab {
    fn drop(&mut self) {
        *self.seat.grabber.borrow_mut() = None;
        self.seat.tree_changed();
    }
}

pub struct WlSeatGlobal {
    name: GlobalName,
    state: Rc<State>,
    seat: Rc<dyn Seat>,
    seat_name: Rc<String>,
    move_: Cell<bool>,
    move_start_pos: Cell<(Fixed, Fixed)>,
    extents_start_pos: Cell<(i32, i32)>,
    pos: Cell<(Fixed, Fixed)>,
    pointer_stack: RefCell<Vec<Rc<dyn Node>>>,
    found_tree: RefCell<Vec<FoundNode>>,
    toplevel_focus_history: LinkedList<Rc<XdgToplevel>>,
    keyboard_node: CloneCell<Rc<dyn Node>>,
    pressed_keys: RefCell<AHashSet<u32>>,
    bindings: RefCell<AHashMap<ClientId, AHashMap<WlSeatId, Rc<WlSeatObj>>>>,
    data_devices: RefCell<AHashMap<ClientId, AHashMap<WlDataDeviceId, Rc<WlDataDevice>>>>,
    kb_state: RefCell<XkbState>,
    layout: Rc<OwnedFd>,
    layout_size: u32,
    cursor: CloneCell<Option<Rc<dyn Cursor>>>,
    serial: NumCell<u32>,
    grabber: RefCell<Option<PointerGrabber>>,
}

impl WlSeatGlobal {
    pub fn new(name: GlobalName, state: &Rc<State>, seat: &Rc<dyn Seat>) -> Self {
        let (kb_state, layout, layout_size) = {
            let ctx = XkbContext::new().unwrap();
            let keymap = ctx.default_keymap().unwrap();
            let state = keymap.state().unwrap();
            let string = keymap.as_str().unwrap();
            let mut memfd =
                uapi::memfd_create("keymap", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING).unwrap();
            memfd.write_all(string.as_bytes()).unwrap();
            memfd.write_all(&[0]).unwrap();
            uapi::lseek(memfd.raw(), 0, c::SEEK_SET).unwrap();
            uapi::fcntl_add_seals(
                memfd.raw(),
                c::F_SEAL_SEAL | c::F_SEAL_GROW | c::F_SEAL_SHRINK | c::F_SEAL_WRITE,
            )
            .unwrap();
            (state, Rc::new(memfd), (string.len() + 1) as _)
        };
        Self {
            name,
            state: state.clone(),
            seat: seat.clone(),
            seat_name: Rc::new(format!("seat-{}", seat.id())),
            move_: Cell::new(false),
            move_start_pos: Cell::new((Fixed(0), Fixed(0))),
            extents_start_pos: Cell::new((0, 0)),
            pos: Cell::new((Fixed(0), Fixed(0))),
            pointer_stack: RefCell::new(vec![]),
            found_tree: RefCell::new(vec![]),
            toplevel_focus_history: Default::default(),
            keyboard_node: CloneCell::new(state.root.clone()),
            pressed_keys: RefCell::new(Default::default()),
            bindings: Default::default(),
            data_devices: RefCell::new(Default::default()),
            kb_state: RefCell::new(kb_state),
            layout,
            layout_size,
            cursor: Default::default(),
            serial: Default::default(),
            grabber: RefCell::new(None),
        }
    }

    pub fn grab_pointer(self: &Rc<Self>, node: Rc<dyn Node>) -> Option<PointerGrab> {
        let mut grabber = self.grabber.borrow_mut();
        if grabber.is_some() {
            return None;
        }
        *grabber = Some(PointerGrabber { node });
        Some(PointerGrab { seat: self.clone() })
    }

    pub fn set_known_cursor(&self, cursor: KnownCursor) {
        let cursors = match self.state.cursors.get() {
            Some(c) => c,
            None => {
                self.set_cursor(None);
                return;
            }
        };
        let tpl = match cursor {
            KnownCursor::Default => &cursors.default,
            KnownCursor::ResizeLeftRight => &cursors.resize_left_right,
            KnownCursor::ResizeTopBottom => &cursors.resize_top_bottom,
        };
        self.set_cursor(Some(tpl.instantiate()));
    }

    pub fn set_cursor(&self, cursor: Option<Rc<dyn Cursor>>) {
        if let Some(old) = self.cursor.get() {
            if let Some(new) = cursor.as_ref() {
                if Rc::ptr_eq(&old, new) {
                    return;
                }
            }
            old.handle_unset();
        }
        if let Some(cursor) = cursor.as_ref() {
            let (x, y) = self.pos.get();
            cursor.set_position(x.round_down(), y.round_down());
        }
        self.cursor.set(cursor);
    }

    pub fn get_cursor(&self) -> Option<Rc<dyn Cursor>> {
        self.cursor.get()
    }

    pub fn id(&self) -> SeatId {
        self.seat.id()
    }

    fn bind_(
        self: Rc<Self>,
        id: WlSeatId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), WlSeatError> {
        let obj = Rc::new(WlSeatObj {
            global: self.clone(),
            id,
            client: client.clone(),
            pointers: Default::default(),
            keyboards: Default::default(),
            version,
        });
        client.add_client_obj(&obj)?;
        client.event(obj.capabilities());
        if version >= SEAT_NAME_SINCE {
            client.event(obj.name(&self.seat_name));
        }
        {
            let mut bindings = self.bindings.borrow_mut();
            let bindings = bindings.entry(client.id).or_insert_with(Default::default);
            bindings.insert(id, obj.clone());
        }
        Ok(())
    }
}

bind!(WlSeatGlobal);

impl Global for WlSeatGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn singleton(&self) -> bool {
        false
    }

    fn interface(&self) -> Interface {
        Interface::WlSeat
    }

    fn version(&self) -> u32 {
        7
    }

    fn break_loops(&self) {
        self.bindings.borrow_mut().clear();
    }
}

pub struct WlSeatObj {
    pub global: Rc<WlSeatGlobal>,
    id: WlSeatId,
    client: Rc<Client>,
    pointers: CopyHashMap<WlPointerId, Rc<WlPointer>>,
    keyboards: CopyHashMap<WlKeyboardId, Rc<WlKeyboard>>,
    version: u32,
}

impl WlSeatObj {
    fn capabilities(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Capabilities {
            obj: self.clone(),
            capabilities: POINTER | KEYBOARD,
        })
    }

    fn name(self: &Rc<Self>, name: &Rc<String>) -> DynEventFormatter {
        Box::new(Name {
            obj: self.clone(),
            name: name.clone(),
        })
    }

    pub fn add_data_device(&self, device: &Rc<WlDataDevice>) {
        let mut dd = self.global.data_devices.borrow_mut();
        dd.entry(self.client.id)
            .or_default()
            .insert(device.id, device.clone());
    }

    pub fn remove_data_device(&self, device: &WlDataDevice) {
        let mut dd = self.global.data_devices.borrow_mut();
        if let Entry::Occupied(mut e) = dd.entry(self.client.id) {
            e.get_mut().remove(&device.id);
            if e.get().is_empty() {
                e.remove();
            }
        }
    }

    pub fn client(&self) -> &Rc<Client> {
        &self.client
    }

    pub fn move_(&self, node: &Rc<FloatNode>) {
        self.global.move_(node);
    }

    fn get_pointer(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetPointerError> {
        let req: GetPointer = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlPointer::new(req.id, self));
        self.client.add_client_obj(&p)?;
        self.pointers.set(req.id, p);
        Ok(())
    }

    fn get_keyboard(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetKeyboardError> {
        let req: GetKeyboard = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlKeyboard::new(req.id, self));
        self.client.add_client_obj(&p)?;
        self.keyboards.set(req.id, p.clone());
        self.client
            .event(p.keymap(wl_keyboard::XKB_V1, p.keymap_fd()?, self.global.layout_size));
        if self.version >= REPEAT_INFO_SINCE {
            self.client.event(p.repeat_info(25, 250));
        }
        Ok(())
    }

    fn get_touch(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetTouchError> {
        let req: GetTouch = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlTouch::new(req.id, self));
        self.client.add_client_obj(&p)?;
        Ok(())
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.client.parse(self, parser)?;
        {
            let mut bindings = self.global.bindings.borrow_mut();
            if let Some(hm) = bindings.get_mut(&self.client.id) {
                hm.remove(&self.id);
            }
        }
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSeatError> {
        match request {
            GET_POINTER => self.get_pointer(parser)?,
            GET_KEYBOARD => self.get_keyboard(parser)?,
            GET_TOUCH => self.get_touch(parser)?,
            RELEASE => self.release(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlSeatObj);

impl Object for WlSeatObj {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlSeat
    }

    fn num_requests(&self) -> u32 {
        if self.version < 5 {
            GET_TOUCH + 1
        } else {
            RELEASE + 1
        }
    }

    fn break_loops(&self) {
        {
            let mut bindings = self.global.bindings.borrow_mut();
            if let Some(hm) = bindings.get_mut(&self.client.id) {
                hm.remove(&self.id);
            }
        }
        self.pointers.clear();
        self.keyboards.clear();
    }
}
