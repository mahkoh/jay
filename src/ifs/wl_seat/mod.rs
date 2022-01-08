mod types;
pub mod wl_keyboard;
pub mod wl_pointer;
pub mod wl_touch;

use crate::backend::{KeyState, OutputId, ScrollAxis, Seat, SeatEvent};
use crate::client::{AddObj, Client, ClientId, DynEventFormatter};
use crate::fixed::Fixed;
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_seat::wl_keyboard::{WlKeyboard, WlKeyboardId};
use crate::ifs::wl_seat::wl_pointer::{WlPointer, WlPointerId};
use crate::ifs::wl_seat::wl_touch::WlTouch;
use crate::object::{Interface, Object, ObjectId};
use crate::tree::{Node, NodeBase, NodeKind, ToplevelNode};
use crate::utils::buffd::MsgParser;
use crate::utils::copyhashmap::CopyHashMap;
use crate::xkbcommon::XkbContext;
use crate::State;
use ahash::AHashMap;
use bstr::ByteSlice;
use std::cell::{Cell, RefCell};
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
const TOUCH: u32 = 4;

const MISSING_CAPABILITY: u32 = 0;

pub struct WlSeatGlobal {
    name: GlobalName,
    state: Rc<State>,
    seat: Rc<dyn Seat>,
    move_: Cell<bool>,
    move_start_pos: Cell<(Fixed, Fixed)>,
    extents_start_pos: Cell<(i32, i32)>,
    pos: Cell<(Fixed, Fixed)>,
    cursor_node: RefCell<Rc<dyn Node>>,
    bindings: RefCell<AHashMap<ClientId, AHashMap<WlSeatId, Rc<WlSeatObj>>>>,
    layout: Rc<OwnedFd>,
    layout_size: u32,
}

impl WlSeatGlobal {
    pub fn new(name: GlobalName, state: &Rc<State>, seat: &Rc<dyn Seat>) -> Self {
        let (layout, layout_size) = {
            let ctx = XkbContext::new().unwrap();
            let keymap = ctx.default_keymap().unwrap();
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
            (Rc::new(memfd), (string.len() + 1) as _)
        };
        Self {
            name,
            state: state.clone(),
            seat: seat.clone(),
            move_: Cell::new(false),
            move_start_pos: Cell::new((Fixed(0), Fixed(0))),
            extents_start_pos: Cell::new((0, 0)),
            pos: Cell::new((Fixed(0), Fixed(0))),
            cursor_node: RefCell::new(state.root.clone()),
            bindings: Default::default(),
            layout,
            layout_size,
        }
    }

    pub fn move_(&self, node: &Rc<ToplevelNode>) {
        let cursor = self.cursor_node.borrow().clone();
        if cursor.id() == node.id() {
            self.move_.set(true);
            self.move_start_pos.set(self.pos.get());
            let ex = node.common.extents.get();
            self.extents_start_pos.set((ex.x, ex.y));
        }
    }

    pub async fn event(&self, event: SeatEvent) {
        match event {
            SeatEvent::OutputPosition(o, x, y) => self.output_position_event(o, x, y).await,
            SeatEvent::Motion(dx, dy) => self.motion_event(dx, dy).await,
            SeatEvent::Button(b, s) => self.button_event(b, s).await,
            SeatEvent::Scroll(d, a) => self.scroll_event(d, a).await,
            SeatEvent::Key(k, s) => self.key_event(k, s).await,
        }
    }

    async fn output_position_event(&self, output: OutputId, mut x: Fixed, mut y: Fixed) {
        let output = match self.state.outputs.get(&output) {
            Some(o) => o,
            _ => return,
        };
        x += Fixed::from_int(output.x.get());
        y += Fixed::from_int(output.y.get());
        self.handle_new_position(x, y).await;
    }

    fn for_each_pointer<C>(&self, client: ClientId, mut f: C)
    where
        C: FnMut(&Rc<WlPointer>),
    {
        let bindings = self.bindings.borrow();
        if let Some(hm) = bindings.get(&client) {
            for seat in hm.values() {
                let pointers = seat.pointers.lock();
                for pointer in pointers.values() {
                    f(pointer);
                }
            }
        }
    }

    async fn tl_pointer_event<F>(&self, tl: &ToplevelNode, mut f: F)
    where
        F: FnMut(&Rc<WlPointer>) -> DynEventFormatter,
    {
        let client = &tl.surface.surface.surface.client;
        self.for_each_pointer(client.id, |p| {
            client.event_locked(f(p));
        });
        let _ = client.flush().await;
    }

    async fn handle_new_position(&self, x: Fixed, y: Fixed) {
        self.pos.set((x, y));
        let cur_node = self.cursor_node.borrow().clone();
        if self.move_.get() {
            if let NodeKind::Toplevel(tn) = cur_node.into_kind() {
                let (move_start_x, move_start_y) = self.move_start_pos.get();
                let (move_start_ex, move_start_ey) = self.extents_start_pos.get();
                let mut ex = tn.common.extents.get();
                ex.x = (x - move_start_x).round_down() + move_start_ex;
                ex.y = (y - move_start_y).round_down() + move_start_ey;
                tn.common.extents.set(ex);
            }
            return;
        }
        let x_int = x.round_down();
        let y_int = y.round_down();
        let (node_dyn, x_int, y_int) = self.state.root.clone().find_node_at(x_int, y_int);
        let mut x = x.apply_fract(x_int);
        let mut y = x.apply_fract(y_int);
        let node = node_dyn.clone().into_kind();
        let mut enter = false;
        if node_dyn.id() != cur_node.id() {
            if let NodeKind::Toplevel(tl) = cur_node.into_kind() {
                self.tl_pointer_event(&tl, |p| p.leave(0, tl.surface.surface.surface.id))
                    .await;
            }
            enter = true;
            *self.cursor_node.borrow_mut() = node_dyn;
        }
        if let NodeKind::Toplevel(tl) = &node {
            let ee = tl.surface.surface.surface.effective_extents.get();
            // log::trace!("{} {}", Fixed::from_int(ee.x1), Fixed::from_int(ee.y1));
            x += Fixed::from_int(ee.x1);
            y += Fixed::from_int(ee.y1);
            if enter {
                self.tl_pointer_event(&tl, |p| p.enter(0, tl.surface.surface.surface.id, x, y))
                    .await;
            }
            self.tl_pointer_event(&tl, |p| p.motion(0, x, y)).await;
        }
    }

    async fn motion_event(&self, dx: Fixed, dy: Fixed) {
        let (x, y) = self.pos.get();
        self.handle_new_position(x + dx, y + dy).await;
    }

    async fn button_event(&self, button: u32, state: KeyState) {
        if state == KeyState::Released {
            self.move_.set(false);
        }
        let node = self.cursor_node.borrow().clone().into_kind();
        if let NodeKind::Toplevel(node) = node {
            let state = match state {
                KeyState::Released => wl_pointer::RELEASED,
                KeyState::Pressed => wl_pointer::PRESSED,
            };
            self.tl_pointer_event(&node, |p| p.button(0, 0, button, state))
                .await;
        }
    }

    async fn scroll_event(&self, delta: i32, axis: ScrollAxis) {
        let node = self.cursor_node.borrow().clone().into_kind();
        if let NodeKind::Toplevel(node) = node {
            let axis = match axis {
                ScrollAxis::Horizontal => wl_pointer::HORIZONTAL_SCROLL,
                ScrollAxis::Vertical => wl_pointer::VERTICAL_SCROLL,
            };
            self.tl_pointer_event(&node, |p| p.axis(0, axis, Fixed::from_int(delta)))
                .await;
        }
    }

    async fn key_event(&self, key: u32, state: KeyState) {}

    async fn bind_(
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
        });
        client.add_client_obj(&obj)?;
        client.event(obj.capabilities()).await?;
        {
            let mut bindings = self.bindings.borrow_mut();
            let bindings = bindings
                .entry(client.id)
                .or_insert_with(|| Default::default());
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
        3
    }

    fn pre_remove(&self) {
        //
    }

    fn break_loops(&self) {
        self.bindings.borrow_mut().clear();
    }
}

pub struct WlSeatObj {
    global: Rc<WlSeatGlobal>,
    id: WlSeatId,
    client: Rc<Client>,
    pointers: CopyHashMap<WlPointerId, Rc<WlPointer>>,
    keyboards: CopyHashMap<WlKeyboardId, Rc<WlKeyboard>>,
}

impl WlSeatObj {
    fn capabilities(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Capabilities {
            obj: self.clone(),
            capabilities: POINTER | KEYBOARD,
        })
    }

    pub fn move_(&self, node: &Rc<ToplevelNode>) {
        self.global.move_(node);
    }

    async fn get_pointer(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), GetPointerError> {
        let req: GetPointer = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlPointer::new(req.id, self));
        self.client.add_client_obj(&p)?;
        self.pointers.set(req.id, p);
        Ok(())
    }

    async fn get_keyboard(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), GetKeyboardError> {
        let req: GetKeyboard = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlKeyboard::new(req.id, self));
        self.client.add_client_obj(&p)?;
        self.keyboards.set(req.id, p.clone());
        self.client
            .event(p.keymap(
                wl_keyboard::XKB_V1,
                self.global.layout.clone(),
                self.global.layout_size,
            ))
            .await?;
        Ok(())
    }

    async fn get_touch(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetTouchError> {
        let req: GetTouch = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlTouch::new(req.id, self));
        self.client.add_client_obj(&p)?;
        Ok(())
    }

    async fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.client.parse(self, parser)?;
        {
            let mut bindings = self.global.bindings.borrow_mut();
            if let Some(hm) = bindings.get_mut(&self.client.id) {
                hm.remove(&self.id);
            }
        }
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSeatError> {
        match request {
            GET_POINTER => self.get_pointer(parser).await?,
            GET_KEYBOARD => self.get_keyboard(parser).await?,
            GET_TOUCH => self.get_touch(parser).await?,
            RELEASE => self.release(parser).await?,
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
        RELEASE + 1
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
