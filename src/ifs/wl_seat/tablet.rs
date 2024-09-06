use {
    crate::{
        backend::{InputDeviceGroupId, InputDeviceId},
        cursor_user::CursorUser,
        ifs::{
            wl_seat::{
                tablet::{
                    pad_owner::PadOwnerHolder, tablet_bindings::TabletBindings,
                    tool_owner::ToolOwnerHolder, zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
                    zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2,
                    zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2,
                    zwp_tablet_pad_v2::ZwpTabletPadV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
                    zwp_tablet_tool_v2::ZwpTabletToolV2, zwp_tablet_v2::ZwpTabletV2,
                },
                WlSeatGlobal,
            },
            wl_surface::WlSurface,
        },
        object::Version,
        tree::{FoundNode, Node},
        utils::{
            bindings::PerClientBindings, clonecell::CloneCell, copyhashmap::CopyHashMap,
            hash_map_ext::HashMapExt,
        },
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

mod pad;
mod pad_owner;
mod tablet_bindings;
mod tool;
pub mod tool_owner;
pub mod zwp_tablet_manager_v2;
pub mod zwp_tablet_pad_group_v2;
pub mod zwp_tablet_pad_ring_v2;
pub mod zwp_tablet_pad_strip_v2;
pub mod zwp_tablet_pad_v2;
pub mod zwp_tablet_seat_v2;
pub mod zwp_tablet_tool_v2;
pub mod zwp_tablet_v2;

#[derive(Default)]
pub struct TabletSeatData {
    seats: PerClientBindings<ZwpTabletSeatV2>,
    tablets: CopyHashMap<TabletId, Rc<Tablet>>,
    tools: CopyHashMap<TabletToolId, Rc<TabletTool>>,
    pads: CopyHashMap<TabletPadId, Rc<TabletPad>>,
}

#[derive(Debug, Clone)]
pub struct TabletInit {
    pub id: TabletId,
    pub group: InputDeviceGroupId,
    pub name: String,
    pub pid: u32,
    pub vid: u32,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct TabletToolInit {
    pub tablet_id: TabletId,
    pub id: TabletToolId,
    pub type_: TabletToolType,
    pub hardware_serial: u64,
    pub hardware_id_wacom: u64,
    pub capabilities: Vec<TabletToolCapability>,
}

#[derive(Debug, Clone)]
pub struct TabletPadInit {
    pub id: TabletPadId,
    pub group: InputDeviceGroupId,
    pub path: String,
    pub buttons: u32,
    pub strips: u32,
    pub rings: u32,
    pub groups: Vec<TabletPadGroupInit>,
}

#[derive(Debug, Clone)]
pub struct TabletPadGroupInit {
    pub buttons: Vec<u32>,
    pub rings: Vec<u32>,
    pub strips: Vec<u32>,
    pub modes: u32,
    pub mode: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PadButtonState {
    Released,
    Pressed,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ToolButtonState {
    Released,
    Pressed,
}

linear_ids!(TabletIds, TabletId);

pub struct Tablet {
    _id: TabletId,
    dev: InputDeviceId,
    group: InputDeviceGroupId,
    name: String,
    pid: u32,
    vid: u32,
    path: String,
    bindings: TabletBindings<ZwpTabletV2>,
    tools: CopyHashMap<TabletToolId, Rc<TabletTool>>,
    pads: CopyHashMap<TabletPadId, Rc<TabletPad>>,
    tree: RefCell<Vec<FoundNode>>,
    seat: Rc<WlSeatGlobal>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TabletToolType {
    Pen,
    Eraser,
    Brush,
    Pencil,
    Airbrush,
    #[expect(dead_code)]
    Finger,
    Mouse,
    Lens,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TabletToolCapability {
    Tilt,
    Pressure,
    Distance,
    Rotation,
    Slider,
    Wheel,
}

linear_ids!(TabletToolIds, TabletToolId, usize);

#[derive(Default)]
pub struct TabletToolOpt {
    tool: CloneCell<Option<Rc<TabletTool>>>,
}

pub struct TabletTool {
    pub id: TabletToolId,
    opt: Rc<TabletToolOpt>,
    tablet: Rc<Tablet>,
    type_: TabletToolType,
    hardware_serial: u64,
    hardware_id_wacom: u64,
    capabilities: Vec<TabletToolCapability>,
    bindings: TabletBindings<ZwpTabletToolV2>,
    node: CloneCell<Rc<dyn Node>>,
    pub(super) tool_owner: ToolOwnerHolder,
    cursor: Rc<CursorUser>,

    down: Cell<bool>,
    pressure: Cell<f64>,
    distance: Cell<f64>,
    tilt_x: Cell<f64>,
    tilt_y: Cell<f64>,
    rotation: Cell<f64>,
    slider: Cell<f64>,
}

linear_ids!(TabletPadIds, TabletPadId);

pub struct TabletPad {
    pub id: TabletPadId,
    dev: InputDeviceId,
    seat: Rc<WlSeatGlobal>,
    group: InputDeviceGroupId,
    tablet: CloneCell<Option<Rc<Tablet>>>,
    path: String,
    buttons: u32,
    bindings: TabletBindings<ZwpTabletPadV2>,
    groups: Vec<Rc<TabletPadGroup>>,
    strips: Vec<Rc<TabletPadStrip>>,
    rings: Vec<Rc<TabletPadRing>>,
    node: CloneCell<Rc<dyn Node>>,
    pub(super) pad_owner: PadOwnerHolder,
}

pub struct TabletPadGroup {
    buttons: Vec<u32>,
    mode: Cell<u32>,
    modes: u32,
    rings: Vec<u32>,
    strips: Vec<u32>,
    bindings: TabletBindings<ZwpTabletPadGroupV2>,
}

pub struct TabletPadStrip {
    bindings: TabletBindings<ZwpTabletPadStripV2>,
}

pub struct TabletPadRing {
    bindings: TabletBindings<ZwpTabletPadRingV2>,
}

#[derive(Copy, Clone, Debug)]
pub enum TabletRingEventSource {
    Finger,
}

#[derive(Copy, Clone, Debug)]
pub enum TabletStripEventSource {
    Finger,
}

#[derive(Debug, Default)]
pub struct TabletToolChanges {
    pub down: Option<bool>,
    pub pos: Option<TabletTool2dChange<TabletToolPositionChange>>,
    pub pressure: Option<f64>,
    pub distance: Option<f64>,
    pub tilt: Option<TabletTool2dChange<f64>>,
    pub rotation: Option<f64>,
    pub slider: Option<f64>,
    pub wheel: Option<TabletToolWheelChange>,
}

#[derive(Copy, Clone, Debug)]
pub struct TabletTool2dChange<T> {
    pub x: T,
    pub y: T,
}

#[derive(Copy, Clone, Debug)]
pub struct TabletToolPositionChange {
    pub x: f64,
    pub dx: f64,
}

#[derive(Copy, Clone, Debug)]
pub struct TabletToolWheelChange {
    pub degrees: f64,
    pub clicks: i32,
}

impl WlSeatGlobal {
    fn tablet_add_seat(&self, seat: &Rc<ZwpTabletSeatV2>) {
        self.tablet.seats.add(&seat.client, seat);
        for tablet in self.tablet.tablets.lock().values() {
            seat.announce_tablet(tablet);
        }
        for tool in self.tablet.tools.lock().values() {
            seat.announce_tool(tool);
        }
        for pad in self.tablet.pads.lock().values() {
            seat.announce_pad(pad);
        }
    }

    pub fn tablet_add_tablet(self: &Rc<Self>, dev: InputDeviceId, init: &TabletInit) {
        let tablet = Rc::new(Tablet {
            _id: init.id,
            dev,
            group: init.group,
            name: init.name.clone(),
            pid: init.pid,
            vid: init.vid,
            path: init.path.clone(),
            bindings: Default::default(),
            tools: Default::default(),
            pads: Default::default(),
            tree: Default::default(),
            seat: self.clone(),
        });
        self.tablet.tablets.set(init.id, tablet.clone());
        self.tablet_for_each_seat_obj(|s| s.announce_tablet(&tablet));
        for pad in self.tablet.pads.lock().values() {
            if pad.tablet.is_none() && pad.group == init.group {
                self.connect_tablet_and_pad(&tablet, pad);
            }
        }
    }

    fn tablet_for_each_seat_obj(&self, mut f: impl FnMut(&Rc<ZwpTabletSeatV2>)) {
        for seats in self.tablet.seats.borrow().values() {
            for seat in seats.values() {
                f(seat);
            }
        }
    }

    pub fn tablet_clear(&self) {
        self.tablet.seats.clear();
        for tablet in self.tablet.tablets.lock().drain_values() {
            tablet.pads.clear();
            tablet.bindings.clear();
            tablet.tools.clear();
        }
        for tool in self.tablet.tools.lock().drain_values() {
            tool.cursor.detach();
            tool.opt.tool.take();
            tool.tool_owner.destroy(&tool);
            tool.bindings.clear();
        }
        for pad in self.tablet.pads.lock().drain_values() {
            pad.pad_owner.destroy(&pad);
            pad.tablet.take();
            pad.bindings.clear();
            for group in &pad.groups {
                group.bindings.clear();
            }
            for rings in &pad.rings {
                rings.bindings.clear();
            }
            for strips in &pad.strips {
                strips.bindings.clear();
            }
        }
    }

    pub fn tablet_remove_tablet(self: &Rc<Self>, id: TabletId) {
        let Some(tablet) = self.tablet.tablets.remove(&id) else {
            return;
        };
        for tool in tablet.tools.lock().drain_values() {
            self.tablet_handle_remove_tool(tablet.seat.state.now_usec(), tool.id);
        }
        for pad in tablet.pads.lock().drain_values() {
            pad.pad_owner.destroy(&pad);
            pad.tablet.take();
        }
        for binding in tablet.bindings.lock().drain_values() {
            binding.send_removed();
        }
    }

    fn connect_tablet_and_pad(self: &Rc<Self>, tablet: &Rc<Tablet>, pad: &Rc<TabletPad>) {
        pad.tablet.set(Some(tablet.clone()));
        tablet.pads.set(pad.id, pad.clone());
        pad.pad_owner.update_node(pad);
    }

    pub fn tablet_on_keyboard_node_change(self: &Rc<Self>) {
        if self.tablet.pads.is_empty() {
            return;
        }
        for pad in self.tablet.pads.lock().values() {
            if pad.tablet.is_some() {
                pad.pad_owner.update_node(pad);
            }
        }
    }

    fn tablet_for_each_seat(&self, surface: &WlSurface, f: impl FnMut(&ZwpTabletSeatV2)) {
        self.tablet
            .seats
            .for_each(surface.client.id, Version::ALL, f)
    }

    pub(super) fn tablet_apply_changes(self: &Rc<Self>) {
        if self.tablet.tools.is_empty() {
            return;
        }
        let now = self.state.now_usec();
        for tool in self.tablet.tools.lock().values() {
            tool.tool_owner.apply_changes(tool, now, None);
        }
    }
}

fn normalizei(n: f64) -> i32 {
    (65535.0 * n) as i32
}

fn normalizeu(n: f64) -> u32 {
    normalizei(n) as u32
}

impl TabletTool {
    pub fn cursor(&self) -> &Rc<CursorUser> {
        &self.cursor
    }

    pub fn node(&self) -> Rc<dyn Node> {
        self.node.get()
    }

    pub fn seat(&self) -> &Rc<WlSeatGlobal> {
        &self.tablet.seat
    }
}
