mod event_handling;
mod kb_owner;
mod pointer_owner;
pub mod wl_keyboard;
pub mod wl_pointer;
pub mod wl_touch;

use crate::async_engine::SpawnedFuture;
use crate::client::{Client, ClientError, ClientId};
use crate::cursor::{Cursor, KnownCursor};
use crate::fixed::Fixed;
use crate::globals::{Global, GlobalName};
use crate::ifs::ipc;
use crate::ifs::ipc::wl_data_device::WlDataDevice;
use crate::ifs::ipc::wl_data_source::WlDataSource;
use crate::ifs::ipc::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use crate::ifs::ipc::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1;
use crate::ifs::ipc::IpcError;
use crate::ifs::wl_output::WlOutputGlobal;
use crate::ifs::wl_seat::kb_owner::KbOwnerHolder;
use crate::ifs::wl_seat::pointer_owner::PointerOwnerHolder;
use crate::ifs::wl_seat::wl_keyboard::{WlKeyboard, WlKeyboardError, REPEAT_INFO_SINCE};
use crate::ifs::wl_seat::wl_pointer::WlPointer;
use crate::ifs::wl_seat::wl_touch::WlTouch;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::{Object, ObjectId};
use crate::tree::toplevel::ToplevelNode;
use crate::tree::{ContainerSplit, FloatNode, FoundNode, Node};
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::{LinkedList, LinkedNode};
use crate::wire::wl_seat::*;
use crate::wire::{
    WlDataDeviceId, WlKeyboardId, WlPointerId, WlSeatId, ZwpPrimarySelectionDeviceV1Id,
};
use crate::xkbcommon::{XkbKeymap, XkbState};
use crate::{ErrorFmt, NumCell, State};
use ahash::{AHashMap, AHashSet};
pub use event_handling::NodeSeatState;
use i4config::keyboard::mods::Modifiers;
use i4config::Direction;
use std::cell::{Cell, RefCell};
use std::collections::hash_map::Entry;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, Errno, OwnedFd};

const POINTER: u32 = 1;
const KEYBOARD: u32 = 2;
#[allow(dead_code)]
const TOUCH: u32 = 4;

#[allow(dead_code)]
const MISSING_CAPABILITY: u32 = 0;

pub const BTN_LEFT: u32 = 0x110;

pub const SEAT_NAME_SINCE: u32 = 2;

#[derive(Clone)]
pub struct Dnd {
    pub seat: Rc<WlSeatGlobal>,
    client: Rc<Client>,
    src: Option<Rc<WlDataSource>>,
}

pub struct DroppedDnd {
    dnd: Dnd,
}

impl Drop for DroppedDnd {
    fn drop(&mut self) {
        if let Some(src) = self.dnd.src.take() {
            ipc::detach_seat::<WlDataDevice>(&src);
        }
    }
}

linear_ids!(SeatIds, SeatId);

pub struct WlSeatGlobal {
    id: SeatId,
    name: GlobalName,
    state: Rc<State>,
    seat_name: String,
    move_: Cell<bool>,
    move_start_pos: Cell<(Fixed, Fixed)>,
    extents_start_pos: Cell<(i32, i32)>,
    pos: Cell<(Fixed, Fixed)>,
    pointer_stack: RefCell<Vec<Rc<dyn Node>>>,
    found_tree: RefCell<Vec<FoundNode>>,
    toplevel_focus_history: LinkedList<Rc<dyn ToplevelNode>>,
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
    repeat_rate: Cell<(i32, i32)>,
    kb_map: CloneCell<Rc<XkbKeymap>>,
    kb_state: RefCell<XkbState>,
    cursor: CloneCell<Option<Rc<dyn Cursor>>>,
    serial: NumCell<u32>,
    tree_changed: Rc<AsyncEvent>,
    selection: CloneCell<Option<Rc<WlDataSource>>>,
    primary_selection: CloneCell<Option<Rc<ZwpPrimarySelectionSourceV1>>>,
    pointer_owner: PointerOwnerHolder,
    kb_owner: KbOwnerHolder,
    dropped_dnd: RefCell<Option<DroppedDnd>>,
    shortcuts: CopyHashMap<(u32, u32), Modifiers>,
    queue_link: Cell<Option<LinkedNode<Rc<Self>>>>,
    tree_changed_handler: Cell<Option<SpawnedFuture<()>>>,
}

impl WlSeatGlobal {
    pub fn new(name: GlobalName, seat_name: &str, state: &Rc<State>) -> Rc<Self> {
        let slf = Rc::new(Self {
            id: state.seat_ids.next(),
            name,
            state: state.clone(),
            seat_name: seat_name.to_string(),
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
            repeat_rate: Cell::new((25, 250)),
            kb_map: CloneCell::new(state.default_keymap.clone()),
            kb_state: RefCell::new(state.default_keymap.state().unwrap()),
            cursor: Default::default(),
            serial: Default::default(),
            tree_changed: Default::default(),
            selection: Default::default(),
            primary_selection: Default::default(),
            pointer_owner: Default::default(),
            kb_owner: Default::default(),
            dropped_dnd: RefCell::new(None),
            shortcuts: Default::default(),
            queue_link: Cell::new(None),
            tree_changed_handler: Cell::new(None),
        });
        let seat = slf.clone();
        let future = state.eng.spawn(async move {
            loop {
                seat.tree_changed.triggered().await;
                seat.state.tree_changed_sent.set(false);
                seat.tree_changed();
            }
        });
        slf.tree_changed_handler.set(Some(future));
        slf
    }

    pub fn get_output(&self) -> Option<Rc<WlOutputGlobal>> {
        let ps = self.pointer_stack.borrow_mut();
        for node in ps.deref() {
            if node.is_output() {
                let on = node.clone().into_output().unwrap();
                return Some(on.global.clone());
            }
        }
        None
    }

    pub fn mark_last_active(self: &Rc<Self>) {
        self.queue_link
            .set(Some(self.state.seat_queue.add_last(self.clone())));
    }

    pub fn set_keymap(&self, keymap: &Rc<XkbKeymap>) {
        self.kb_map.set(keymap.clone());
        let bindings = self.bindings.borrow_mut();
        for (id, client) in bindings.iter() {
            for seat in client.values() {
                let kbs = seat.keyboards.lock();
                for kb in kbs.values() {
                    let fd = match seat.keymap_fd(&keymap) {
                        Ok(fd) => fd,
                        Err(e) => {
                            log::error!("Could not creat a file descriptor to transfer the keymap to client {}: {}", id, ErrorFmt(e));
                            continue;
                        }
                    };
                    kb.send_keymap(wl_keyboard::XKB_V1, fd, keymap.map_len as _);
                }
            }
        }
    }

    pub fn get_split(&self) -> Option<ContainerSplit> {
        self.keyboard_node.get().get_parent_split()
    }

    pub fn set_split(&self, axis: ContainerSplit) {
        self.keyboard_node.get().set_parent_split(axis)
    }

    pub fn create_split(&self, axis: ContainerSplit) {
        self.keyboard_node.get().create_split(axis)
    }

    pub fn focus_parent(self: &Rc<Self>) {
        self.keyboard_node.get().focus_parent(self);
    }

    pub fn toggle_floating(self: &Rc<Self>) {
        self.keyboard_node.get().toggle_floating(self);
    }

    pub fn get_rate(&self) -> (i32, i32) {
        self.repeat_rate.get()
    }

    pub fn set_rate(&self, rate: i32, delay: i32) {
        self.repeat_rate.set((rate, delay));
        let bindings = self.bindings.borrow_mut();
        for client in bindings.values() {
            for seat in client.values() {
                if seat.version >= REPEAT_INFO_SINCE {
                    let kbs = seat.keyboards.lock();
                    for kb in kbs.values() {
                        kb.send_repeat_info(rate, delay);
                    }
                }
            }
        }
    }

    pub fn move_focus(self: &Rc<Self>, direction: Direction) {
        let kb_node = self.keyboard_node.get();
        kb_node.move_focus(self, direction);
    }

    pub fn move_focused(self: &Rc<Self>, direction: Direction) {
        let kb_node = self.keyboard_node.get();
        kb_node.move_self(direction);
    }

    fn set_selection_<T: ipc::Vtable>(
        self: &Rc<Self>,
        field: &CloneCell<Option<Rc<T::Source>>>,
        src: Option<Rc<T::Source>>,
    ) -> Result<(), WlSeatError> {
        if let Some(new) = &src {
            ipc::attach_seat::<T>(new, self, ipc::Role::Selection)?;
        }
        if let Some(old) = field.set(src.clone()) {
            ipc::detach_seat::<T>(&old);
        }
        if let Some(client) = self.keyboard_node.get().client() {
            match src {
                Some(src) => ipc::offer_source_to::<T>(&src, &client),
                _ => T::for_each_device(self, client.id, |device| {
                    T::send_selection(device, ObjectId::NONE.into());
                }),
            }
            client.flush();
        }
        Ok(())
    }

    pub fn start_drag(
        self: &Rc<Self>,
        origin: &Rc<WlSurface>,
        source: Option<Rc<WlDataSource>>,
        icon: Option<Rc<WlSurface>>,
    ) -> Result<(), WlSeatError> {
        self.pointer_owner.start_drag(self, origin, source, icon)
    }

    pub fn cancel_dnd(self: &Rc<Self>) {
        self.pointer_owner.cancel_dnd(self);
    }

    pub fn unset_selection(self: &Rc<Self>) {
        let _ = self.set_selection(None);
    }

    pub fn set_selection(
        self: &Rc<Self>,
        selection: Option<Rc<WlDataSource>>,
    ) -> Result<(), WlSeatError> {
        self.set_selection_::<WlDataDevice>(&self.selection, selection)
    }

    pub fn unset_primary_selection(self: &Rc<Self>) {
        let _ = self.set_primary_selection(None);
    }

    pub fn set_primary_selection(
        self: &Rc<Self>,
        selection: Option<Rc<ZwpPrimarySelectionSourceV1>>,
    ) -> Result<(), WlSeatError> {
        self.set_selection_::<ZwpPrimarySelectionDeviceV1>(&self.primary_selection, selection)
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
            KnownCursor::ResizeTopLeft => &cursors.resize_top_left,
            KnownCursor::ResizeTopRight => &cursors.resize_top_right,
            KnownCursor::ResizeBottomLeft => &cursors.resize_bottom_left,
            KnownCursor::ResizeBottomRight => &cursors.resize_bottom_right,
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

    pub fn dnd_icon(&self) -> Option<Rc<WlSurface>> {
        self.pointer_owner.dnd_icon()
    }

    pub fn remove_dnd_icon(&self) {
        self.pointer_owner.remove_dnd_icon();
    }

    pub fn get_cursor(&self) -> Option<Rc<dyn Cursor>> {
        self.cursor.get()
    }

    pub fn clear(&self) {
        mem::take(self.pointer_stack.borrow_mut().deref_mut());
        mem::take(self.found_tree.borrow_mut().deref_mut());
    }

    pub fn id(&self) -> SeatId {
        self.id
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
            tracker: Default::default(),
        });
        track!(client, obj);
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
        self.queue_link.take();
        self.tree_changed_handler.take();
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
    tracker: Tracker<Self>,
}

const READ_ONLY_KEYMAP_SINCE: u32 = 7;

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
        track!(self.client, p);
        self.client.add_client_obj(&p)?;
        self.pointers.set(req.id, p);
        Ok(())
    }

    fn get_keyboard(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetKeyboardError> {
        let req: GetKeyboard = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlKeyboard::new(req.id, self));
        track!(self.client, p);
        self.client.add_client_obj(&p)?;
        self.keyboards.set(req.id, p.clone());
        let keymap = self.global.kb_map.get();
        p.send_keymap(
            wl_keyboard::XKB_V1,
            self.keymap_fd(&keymap)?,
            keymap.map_len as _,
        );
        if self.version >= REPEAT_INFO_SINCE {
            let (rate, delay) = self.global.repeat_rate.get();
            p.send_repeat_info(rate, delay);
        }
        Ok(())
    }

    pub fn keymap_fd(&self, keymap: &XkbKeymap) -> Result<Rc<OwnedFd>, WlKeyboardError> {
        if self.version >= READ_ONLY_KEYMAP_SINCE {
            return Ok(keymap.map.clone());
        }
        let fd = match uapi::memfd_create("shared-keymap", c::MFD_CLOEXEC) {
            Ok(fd) => fd,
            Err(e) => return Err(WlKeyboardError::KeymapMemfd(e.into())),
        };
        let target = keymap.map_len as c::off_t;
        let mut pos = 0;
        while pos < target {
            let rem = target - pos;
            let res = uapi::sendfile(fd.raw(), keymap.map.raw(), Some(&mut pos), rem as usize);
            match res {
                Ok(_) | Err(Errno(c::EINTR)) => {}
                Err(e) => return Err(WlKeyboardError::KeymapCopy(e.into())),
            }
        }
        Ok(Rc::new(fd))
    }

    fn get_touch(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetTouchError> {
        let req: GetTouch = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlTouch::new(req.id, self));
        track!(self.client, p);
        self.client.add_client_obj(&p)?;
        Ok(())
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.client.parse(self, parser)?;
        {
            let mut bindings = self.global.bindings.borrow_mut();
            if let Entry::Occupied(mut hm) = bindings.entry(self.client.id) {
                hm.get_mut().remove(&self.id);
                if hm.get().is_empty() {
                    hm.remove();
                }
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
            if let Entry::Occupied(mut hm) = bindings.entry(self.client.id) {
                hm.get_mut().remove(&self.id);
                if hm.get().is_empty() {
                    hm.remove();
                }
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
    #[error(transparent)]
    IpcError(#[from] IpcError),
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
