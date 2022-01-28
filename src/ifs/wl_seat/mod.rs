mod types;
pub mod wl_keyboard;
pub mod wl_pointer;
pub mod wl_touch;

use crate::backend::{KeyState, OutputId, ScrollAxis, Seat, SeatEvent};
use crate::client::{Client, ClientId, DynEventFormatter};
use crate::fixed::Fixed;
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_seat::wl_keyboard::{WlKeyboard, WlKeyboardId};
use crate::ifs::wl_seat::wl_pointer::{WlPointer, WlPointerId, POINTER_FRAME_SINCE_VERSION};
use crate::ifs::wl_seat::wl_touch::WlTouch;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::{XdgToplevel, XdgToplevelId};
use crate::ifs::wl_surface::WlSurface;
use crate::object::{Interface, Object, ObjectId};
use crate::tree::{FloatNode, Node};
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::{LinkedList, LinkedNode};
use crate::xkbcommon::{ModifierState, XkbContext, XkbState};
use crate::State;
use ahash::{AHashMap, AHashSet};
use bstr::ByteSlice;
use std::cell::{Cell, RefCell};
use std::io::Write;
use std::ops::Deref;
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

#[allow(dead_code)]
const BTN_LEFT: u32 = 0x110;

pub struct WlSeatGlobal {
    name: GlobalName,
    state: Rc<State>,
    _seat: Rc<dyn Seat>,
    move_: Cell<bool>,
    move_start_pos: Cell<(Fixed, Fixed)>,
    extents_start_pos: Cell<(i32, i32)>,
    pos: Cell<(Fixed, Fixed)>,
    pointer_stack: RefCell<Vec<Rc<dyn Node>>>,
    toplevel_focus_history: LinkedList<Rc<XdgToplevel>>,
    toplevel_focus_stash: RefCell<AHashMap<XdgToplevelId, LinkedNode<Rc<XdgToplevel>>>>,
    keyboard_node: CloneCell<Rc<dyn Node>>,
    pressed_keys: RefCell<AHashSet<u32>>,
    bindings: RefCell<AHashMap<ClientId, AHashMap<WlSeatId, Rc<WlSeatObj>>>>,
    kb_state: RefCell<XkbState>,
    layout: Rc<OwnedFd>,
    layout_size: u32,
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
            _seat: seat.clone(),
            move_: Cell::new(false),
            move_start_pos: Cell::new((Fixed(0), Fixed(0))),
            extents_start_pos: Cell::new((0, 0)),
            pos: Cell::new((Fixed(0), Fixed(0))),
            pointer_stack: RefCell::new(vec![]),
            toplevel_focus_history: Default::default(),
            toplevel_focus_stash: RefCell::new(Default::default()),
            keyboard_node: CloneCell::new(state.root.clone()),
            pressed_keys: RefCell::new(Default::default()),
            bindings: Default::default(),
            kb_state: RefCell::new(kb_state),
            layout,
            layout_size,
        }
    }

    pub fn last_tiled_keyboard_toplevel(&self) -> Option<Rc<XdgToplevel>> {
        for tl in self.toplevel_focus_history.rev_iter() {
            if !tl.parent_is_float() {
                return Some(tl.deref().clone());
            }
        }
        None
    }

    pub fn move_(&self, node: &Rc<FloatNode>) {
        self.move_.set(true);
        self.move_start_pos.set(self.pos.get());
        let ex = node.position.get();
        self.extents_start_pos.set((ex.x1(), ex.y1()));
    }

    pub fn event(&self, event: SeatEvent) {
        match event {
            SeatEvent::OutputPosition(o, x, y) => self.output_position_event(o, x, y),
            SeatEvent::Motion(dx, dy) => self.motion_event(dx, dy),
            SeatEvent::Button(b, s) => self.button_event(b, s),
            SeatEvent::Scroll(d, a) => self.scroll_event(d, a),
            SeatEvent::Key(k, s) => self.key_event(k, s),
        }
    }

    pub fn button_surface(&self, surface: &Rc<WlSurface>, button: u32, state: KeyState) {
        let state = match state {
            KeyState::Released => wl_pointer::RELEASED,
            KeyState::Pressed => wl_pointer::PRESSED,
        };
        self.surface_pointer_event(0, surface, |p| p.button(0, 0, button, state));
    }

    pub fn focus_surface(&self, surface: &Rc<WlSurface>) {
        let pressed_keys: Vec<_> = self.pressed_keys.borrow().iter().copied().collect();
        self.surface_kb_event(0, &surface, |k| {
            k.enter(0, surface.id, pressed_keys.clone())
        });
        let ModifierState {
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        } = self.kb_state.borrow().mods();
        self.surface_kb_event(0, surface, |k| {
            k.modifiers(0, mods_depressed, mods_latched, mods_locked, group)
        });
    }

    pub fn unfocus_surface(&self, surface: &Rc<WlSurface>) {
        self.surface_kb_event(0, surface, |k| k.leave(0, surface.id))
    }

    fn focus_toplevel(&self, toplevel: &Rc<XdgToplevel>) {
        let node = self.toplevel_focus_history.add_last(toplevel.clone());
        self.toplevel_focus_stash
            .borrow_mut()
            .insert(toplevel.id, node);
        self.keyboard_node.get().unfocus(self);
        let focus_surface;
        if let Some(ss) = toplevel.focus_subsurface.get() {
            focus_surface = ss.surface.clone();
            self.keyboard_node.set(ss);
        } else {
            focus_surface = toplevel.xdg.surface.clone();
            self.keyboard_node.set(focus_surface.clone());
        }
        self.focus_surface(&focus_surface);
    }

    fn output_position_event(&self, output: OutputId, mut x: Fixed, mut y: Fixed) {
        let output = match self.state.outputs.get(&output) {
            Some(o) => o,
            _ => return,
        };
        x += Fixed::from_int(output.x.get());
        y += Fixed::from_int(output.y.get());
        self.set_new_position(x, y);
    }

    fn for_each_seat<C>(&self, ver: u32, client: ClientId, mut f: C)
    where
        C: FnMut(&Rc<WlSeatObj>),
    {
        let bindings = self.bindings.borrow();
        if let Some(hm) = bindings.get(&client) {
            for seat in hm.values() {
                if seat.version >= ver {
                    f(seat);
                }
            }
        }
    }

    fn for_each_pointer<C>(&self, ver: u32, client: ClientId, mut f: C)
    where
        C: FnMut(&Rc<WlPointer>),
    {
        self.for_each_seat(ver, client, |seat| {
            let pointers = seat.pointers.lock();
            for pointer in pointers.values() {
                f(pointer);
            }
        })
    }

    fn for_each_kb<C>(&self, ver: u32, client: ClientId, mut f: C)
    where
        C: FnMut(&Rc<WlKeyboard>),
    {
        self.for_each_seat(ver, client, |seat| {
            let keyboards = seat.keyboards.lock();
            for keyboard in keyboards.values() {
                f(keyboard);
            }
        })
    }

    fn surface_pointer_event<F>(&self, ver: u32, surface: &WlSurface, mut f: F)
    where
        F: FnMut(&Rc<WlPointer>) -> DynEventFormatter,
    {
        let client = &surface.client;
        self.for_each_pointer(ver, client.id, |p| {
            client.event(f(p));
        });
        client.flush();
    }

    fn surface_kb_event<F>(&self, ver: u32, surface: &WlSurface, mut f: F)
    where
        F: FnMut(&Rc<WlKeyboard>) -> DynEventFormatter,
    {
        let client = &surface.client;
        self.for_each_kb(ver, client.id, |p| {
            client.event(f(p));
        });
        client.flush();
    }

    fn set_new_position(&self, x: Fixed, y: Fixed) {
        self.pos.set((x, y));
        self.handle_new_position(true);
    }

    pub fn tree_changed(&self) {
        log::info!("tree changed");
        self.handle_new_position(false);
    }

    pub fn handle_new_position(&self, changed: bool) {
        let (x, y) = self.pos.get();
        let mut stack = self.pointer_stack.borrow_mut();
        // if self.move_.get() {
        //     for node in stack.iter().rev() {
        //         if let NodeKind::Toplevel(tn) = node.clone().into_kind() {
        //             let (move_start_x, move_start_y) = self.move_start_pos.get();
        //             let (move_start_ex, move_start_ey) = self.extents_start_pos.get();
        //             let mut ex = tn.common.extents.get();
        //             ex.x = (x - move_start_x).round_down() + move_start_ex;
        //             ex.y = (y - move_start_y).round_down() + move_start_ey;
        //             tn.common.extents.set(ex);
        //         }
        //     }
        //     return;
        // }
        let mut x_int = x.round_down();
        let mut y_int = y.round_down();
        let mut node = Some(self.state.root.clone() as Rc<dyn Node>);
        let divergence = 'outer: loop {
            for i in 0..stack.len() {
                match node.take() {
                    None => break 'outer i,
                    Some(n) if n.id() != stack[i].id() => {
                        node = Some(n);
                        break 'outer i;
                    }
                    Some(n) => {
                        if let Some(found) = n.find_child_at(x_int.into(), y_int.into()) {
                            node = Some(found.node);
                            x_int = found.x.into();
                            y_int = found.y.into();
                        }
                    }
                }
            }
            break stack.len();
        };
        if divergence == stack.len() && node.is_none() {
            if changed {
                if let Some(node) = stack.last() {
                    node.motion(self, x.apply_fract(x_int), y.apply_fract(y_int));
                }
            }
        } else {
            for node in stack.drain(divergence..).rev() {
                node.leave(self);
            }
            while let Some(n) = node.take() {
                n.clone()
                    .enter(self, x.apply_fract(x_int), y.apply_fract(y_int));
                if let Some(found) = n.find_child_at(x_int.into(), y_int.into()) {
                    node = Some(found.node);
                    x_int = found.x.into();
                    y_int = found.y.into();
                }
                stack.push(n);
            }
        }
    }

    pub fn leave_surface(&self, n: &WlSurface) {
        self.surface_pointer_event(0, n, |p| p.leave(0, n.id));
    }

    pub fn enter_toplevel(&self, n: &Rc<XdgToplevel>) {
        self.focus_toplevel(n);
    }

    pub fn enter_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        self.surface_pointer_event(0, n, |p| p.enter(0, n.id, x, y));
    }

    pub fn motion_surface(&self, n: &WlSurface, x: Fixed, y: Fixed) {
        self.surface_pointer_event(0, n, |p| p.motion(0, x, y));
        self.surface_pointer_event(POINTER_FRAME_SINCE_VERSION, n, |p| p.frame());
    }

    fn motion_event(&self, dx: Fixed, dy: Fixed) {
        let (x, y) = self.pos.get();
        self.set_new_position(x + dx, y + dy);
    }

    fn button_event(&self, button: u32, state: KeyState) {
        if state == KeyState::Released {
            self.move_.set(false);
        }
        let node = match self.pointer_stack.borrow().last().cloned() {
            Some(v) => v,
            _ => return,
        };
        let mut enter = false;
        {
            let kb_node = self.keyboard_node.get();
            if kb_node.id() != node.id() {
                enter = true;
                kb_node.unfocus(self);
                self.keyboard_node.set(node.clone());
            }
        }
        node.clone().button(self, button, state);
        if enter {
            node.focus(self);
        }
    }

    pub fn scroll_surface(&self, surface: &WlSurface, delta: i32, axis: ScrollAxis) {
        let axis = match axis {
            ScrollAxis::Horizontal => wl_pointer::HORIZONTAL_SCROLL,
            ScrollAxis::Vertical => wl_pointer::VERTICAL_SCROLL,
        };
        self.surface_pointer_event(0, surface, |p| p.axis(0, axis, Fixed::from_int(delta)));
        self.surface_pointer_event(POINTER_FRAME_SINCE_VERSION, surface, |p| p.frame());
    }

    fn scroll_event(&self, delta: i32, axis: ScrollAxis) {
        if let Some(node) = self.pointer_stack.borrow().last().cloned() {
            node.scroll(self, delta, axis);
        }
    }

    fn key_event(&self, _key: u32, _state: KeyState) {
        // let (state, xkb_dir) = {
        //     let mut pk = self.pressed_keys.borrow_mut();
        //     match state {
        //         KeyState::Released => {
        //             if !pk.remove(&key) {
        //                 return;
        //             }
        //             (wl_keyboard::RELEASED, XKB_KEY_UP)
        //         }
        //         KeyState::Pressed => {
        //             if !pk.insert(key) {
        //                 return;
        //             }
        //             (wl_keyboard::PRESSED, XKB_KEY_DOWN)
        //         }
        //     }
        // };
        // let mods = self.kb_state.borrow_mut().update(key, xkb_dir);
        // let node = self.keyboard_node.get().into_kind();
        // if let NodeKind::Toplevel(node) = node {
        //     self.tl_kb_event(&node, |k| k.key(0, 0, key, state)).await;
        //     if let Some(mods) = mods {
        //         self.tl_kb_event(&node, |k| {
        //             k.modifiers(
        //                 0,
        //                 mods.mods_depressed,
        //                 mods.mods_latched,
        //                 mods.mods_locked,
        //                 mods.group,
        //             )
        //         })
        //         .await;
        //     }
        // }
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
    global: Rc<WlSeatGlobal>,
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
        self.client.event(p.repeat_info(25, 250));
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
