mod handling;
pub mod wl_keyboard;
pub mod wl_pointer;
pub mod wl_touch;

use crate::backend::{Seat, SeatId};
use crate::client::{Client, ClientError, ClientId};
use crate::cursor::{Cursor, KnownCursor};
use crate::fixed::Fixed;
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_data_device::{WlDataDevice};
use crate::ifs::wl_data_offer::{DataOfferRole};
use crate::ifs::wl_data_source::{WlDataSource, WlDataSourceError};
use crate::ifs::wl_seat::wl_keyboard::{WlKeyboard, REPEAT_INFO_SINCE, WlKeyboardError};
use crate::ifs::wl_seat::wl_pointer::{WlPointer};
use crate::ifs::wl_seat::wl_touch::WlTouch;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::zwp_primary_selection_device_v1::{
    ZwpPrimarySelectionDeviceV1,
};
use crate::ifs::zwp_primary_selection_source_v1::{ZwpPrimarySelectionSourceV1, ZwpPrimarySelectionSourceV1Error};
use crate::object::Object;
use crate::tree::{FloatNode, FoundNode, Node};
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::LinkedList;
use crate::utils::smallmap::SmallMap;
use crate::xkbcommon::{XkbContext, XkbState};
use crate::{NumCell, State};
use ahash::{AHashMap, AHashSet};
use bstr::ByteSlice;
pub use handling::NodeSeatState;
use std::cell::{Cell, RefCell};
use std::collections::hash_map::Entry;
use std::io::Write;
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, OwnedFd};
use crate::wire::wl_seat::*;
use crate::utils::buffd::MsgParserError;
use crate::wire::{WlDataDeviceId, WlDataOfferId, WlKeyboardId, WlPointerId, WlSeatId, ZwpPrimarySelectionDeviceV1Id, ZwpPrimarySelectionOfferV1Id};

const POINTER: u32 = 1;
const KEYBOARD: u32 = 2;
#[allow(dead_code)]
const TOUCH: u32 = 4;

#[allow(dead_code)]
const MISSING_CAPABILITY: u32 = 0;

pub const BTN_LEFT: u32 = 0x110;

pub const SEAT_NAME_SINCE: u32 = 2;

struct PointerGrab {
    seat: Rc<WlSeatGlobal>,
}

struct PointerGrabber {
    node: Rc<dyn Node>,
    buttons: SmallMap<u32, (), 1>,
}

impl Drop for PointerGrab {
    fn drop(&mut self) {
        *self.seat.grabber.borrow_mut() = None;
        self.seat.tree_changed.trigger();
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
    bindings: RefCell<AHashMap<ClientId, AHashMap<WlSeatId, Rc<WlSeat>>>>,
    data_devices: RefCell<AHashMap<ClientId, AHashMap<WlDataDeviceId, Rc<WlDataDevice>>>>,
    primary_selection_devices: RefCell<
        AHashMap<
            ClientId,
            AHashMap<ZwpPrimarySelectionDeviceV1Id, Rc<ZwpPrimarySelectionDeviceV1>>,
        >,
    >,
    kb_state: RefCell<XkbState>,
    layout: Rc<OwnedFd>,
    layout_size: u32,
    cursor: CloneCell<Option<Rc<dyn Cursor>>>,
    serial: NumCell<u32>,
    grabber: RefCell<Option<PointerGrabber>>,
    tree_changed: Rc<AsyncEvent>,
    selection: CloneCell<Option<Rc<WlDataSource>>>,
    primary_selection: CloneCell<Option<Rc<ZwpPrimarySelectionSourceV1>>>,
}

impl WlSeatGlobal {
    pub fn new(
        name: GlobalName,
        state: &Rc<State>,
        seat: &Rc<dyn Seat>,
        tree_changed: &Rc<AsyncEvent>,
    ) -> Self {
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
            primary_selection_devices: RefCell::new(Default::default()),
            kb_state: RefCell::new(kb_state),
            layout,
            layout_size,
            cursor: Default::default(),
            serial: Default::default(),
            grabber: RefCell::new(None),
            tree_changed: tree_changed.clone(),
            selection: Default::default(),
            primary_selection: Default::default(),
        }
    }

    pub fn set_selection(
        self: &Rc<Self>,
        selection: Option<Rc<WlDataSource>>,
    ) -> Result<(), WlDataSourceError> {
        if let Some(new) = &selection {
            new.attach(self, DataOfferRole::Selection)?;
        }
        if let Some(old) = self.selection.set(selection.clone()) {
            old.detach();
        }
        if let Some(client) = self.keyboard_node.get().client() {
            match selection {
                Some(sel) => {
                    sel.create_offer(&client);
                }
                _ => {
                    self.for_each_data_device(0, client.id, |device| {
                        device.send_selection(WlDataOfferId::NONE);
                    });
                }
            }
            client.flush();
        }
        Ok(())
    }

    pub fn set_primary_selection(
        self: &Rc<Self>,
        selection: Option<Rc<ZwpPrimarySelectionSourceV1>>,
    ) -> Result<(), ZwpPrimarySelectionSourceV1Error> {
        if let Some(new) = &selection {
            new.attach(self)?;
        }
        if let Some(old) = self.primary_selection.set(selection.clone()) {
            old.detach();
        }
        if let Some(client) = self.keyboard_node.get().client() {
            match selection {
                Some(sel) => {
                    sel.create_offer(&client);
                }
                _ => {
                    self.for_each_primary_selection_device(0, client.id, |device| {
                        device.send_selection(ZwpPrimarySelectionOfferV1Id::NONE);
                    });
                }
            }
            client.flush();
        }
        Ok(())
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
        let obj = Rc::new(WlSeat {
            global: self.clone(),
            id,
            client: client.clone(),
            pointers: Default::default(),
            keyboards: Default::default(),
            version,
        });
        client.add_client_obj(&obj)?;
        obj.send_capabilities();
        if version >= SEAT_NAME_SINCE {
            obj.send_name(&self.seat_name);
        }
        {
            let mut bindings = self.bindings.borrow_mut();
            let bindings = bindings.entry(client.id).or_insert_with(Default::default);
            bindings.insert(id, obj.clone());
        }
        Ok(())
    }
}

global_base!(WlSeatGlobal, WlSeat, WlSeatError);

impl Global for WlSeatGlobal {
    fn singleton(&self) -> bool {
        false
    }

    fn version(&self) -> u32 {
        7
    }

    fn break_loops(&self) {
        self.bindings.borrow_mut().clear();
    }
}

dedicated_add_global!(WlSeatGlobal, seats);

pub struct WlSeat {
    pub global: Rc<WlSeatGlobal>,
    pub id: WlSeatId,
    pub client: Rc<Client>,
    pointers: CopyHashMap<WlPointerId, Rc<WlPointer>>,
    keyboards: CopyHashMap<WlKeyboardId, Rc<WlKeyboard>>,
    version: u32,
}

impl WlSeat {
    fn send_capabilities(self: &Rc<Self>) {
        self.client.event(Capabilities {
            self_id: self.id,
            capabilities: POINTER | KEYBOARD,
        })
    }

    fn send_name(self: &Rc<Self>, name: &str) {
        self.client.event(Name {
            self_id: self.id,
            name,
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

    pub fn add_primary_selection_device(&self, device: &Rc<ZwpPrimarySelectionDeviceV1>) {
        let mut dd = self.global.primary_selection_devices.borrow_mut();
        dd.entry(self.client.id)
            .or_default()
            .insert(device.id, device.clone());
    }

    pub fn remove_primary_selection_device(&self, device: &ZwpPrimarySelectionDeviceV1) {
        let mut dd = self.global.primary_selection_devices.borrow_mut();
        if let Entry::Occupied(mut e) = dd.entry(self.client.id) {
            e.get_mut().remove(&device.id);
            if e.get().is_empty() {
                e.remove();
            }
        }
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
        p.send_keymap(wl_keyboard::XKB_V1, p.keymap_fd()?, self.global.layout_size);
        if self.version >= REPEAT_INFO_SINCE {
            p.send_repeat_info(25, 250);
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
}

object_base! {
    WlSeat, WlSeatError;

    GET_POINTER => get_pointer,
    GET_KEYBOARD => get_keyboard,
    GET_TOUCH => get_touch,
    RELEASE => release,
}

impl Object for WlSeat {
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

dedicated_add_obj!(WlSeat, WlSeatId, seats);

#[derive(Debug, Error)]
pub enum WlSeatError {
    #[error("Could not handle `get_pointer` request")]
    GetPointerError(#[from] GetPointerError),
    #[error("Could not handle `get_keyboard` request")]
    GetKeyboardError(#[from] GetKeyboardError),
    #[error("Could not handle `get_touch` request")]
    GetTouchError(#[from] GetTouchError),
    #[error("Could not handle `release` request")]
    ReleaseError(#[from] ReleaseError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlSeatError, ClientError);

#[derive(Debug, Error)]
pub enum GetPointerError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetPointerError, ClientError);
efrom!(GetPointerError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum GetKeyboardError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlKeyboardError(Box<WlKeyboardError>),
}
efrom!(GetKeyboardError, ClientError);
efrom!(GetKeyboardError, ParseError, MsgParserError);
efrom!(GetKeyboardError, WlKeyboardError, WlKeyboardError);

#[derive(Debug, Error)]
pub enum GetTouchError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GetTouchError, ClientError, ClientError);
efrom!(GetTouchError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ClientError, ClientError);
efrom!(ReleaseError, ParseError, MsgParserError);
