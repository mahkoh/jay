use {
    crate::{
        backend::InputDeviceId,
        fixed::Fixed,
        ifs::{
            wl_seat::{
                WlSeatGlobal,
                tablet::{
                    PadButtonState, TabletPad, TabletPadGroup, TabletPadId, TabletPadInit,
                    TabletPadRing, TabletPadStrip, TabletRingEventSource, TabletStripEventSource,
                    normalizeu, zwp_tablet_pad_v2::ZwpTabletPadV2, zwp_tablet_v2::ZwpTabletV2,
                },
            },
            wl_surface::WlSurface,
        },
        time::usec_to_msec,
        utils::{clonecell::CloneCell, hash_map_ext::HashMapExt},
    },
    std::{cell::Cell, rc::Rc},
};

impl WlSeatGlobal {
    pub fn tablet_add_tablet_pad(self: &Rc<Self>, dev: InputDeviceId, init: &TabletPadInit) {
        let mut strips = Vec::new();
        for _ in 0..init.strips {
            strips.push(Rc::new(TabletPadStrip {
                bindings: Default::default(),
            }));
        }
        let mut rings = Vec::new();
        for _ in 0..init.rings {
            rings.push(Rc::new(TabletPadRing {
                bindings: Default::default(),
            }));
        }
        let mut groups = Vec::new();
        for group_init in &init.groups {
            groups.push(Rc::new(TabletPadGroup {
                buttons: group_init.buttons.clone(),
                mode: Cell::new(group_init.mode),
                modes: group_init.modes,
                rings: group_init.rings.clone(),
                strips: group_init.strips.clone(),
                bindings: Default::default(),
            }));
        }
        let pad = Rc::new(TabletPad {
            id: init.id,
            dev,
            seat: self.clone(),
            group: init.group,
            tablet: Default::default(),
            path: init.path.clone(),
            buttons: init.buttons,
            bindings: Default::default(),
            groups,
            strips,
            rings,
            node: CloneCell::new(self.state.root.clone()),
            pad_owner: Default::default(),
        });
        self.tablet.pads.set(init.id, pad.clone());
        self.tablet_for_each_seat_obj(|s| s.announce_pad(&pad));
        for tablet in self.tablet.tablets.lock().values() {
            if tablet.group == init.group {
                self.connect_tablet_and_pad(tablet, &pad);
            }
        }
    }

    pub fn tablet_remove_tablet_pad(self: &Rc<Self>, id: TabletPadId) {
        let Some(pad) = self.tablet.pads.remove(&id) else {
            return;
        };
        pad.pad_owner.destroy(&pad);
        if let Some(tablet) = pad.tablet.take() {
            tablet.pads.remove(&pad.id);
        }
        for binding in pad.bindings.lock().drain_values() {
            binding.send_removed();
        }
    }

    pub fn tablet_event_pad_mode_switch(
        self: &Rc<Self>,
        pad: TabletPadId,
        time_usec: u64,
        group_idx: u32,
        mode: u32,
    ) {
        if let Some(pad) = self.tablet.pads.get(&pad) {
            if let Some(group) = pad.groups.get(group_idx as usize) {
                if group.mode.replace(mode) != mode {
                    self.state.for_each_seat_tester(|t| {
                        t.send_tablet_pad_mode_switch(self.id, pad.dev, time_usec, group_idx, mode)
                    });
                    if pad.tablet.is_some() {
                        let node = pad.node.get();
                        node.node_on_tablet_pad_mode_switch(&pad, group, time_usec, mode);
                    }
                }
            }
        }
    }

    pub fn tablet_event_pad_button(
        self: &Rc<Self>,
        pad: TabletPadId,
        time_usec: u64,
        button: u32,
        state: PadButtonState,
    ) {
        if let Some(pad) = self.tablet.pads.get(&pad) {
            self.state.for_each_seat_tester(|t| {
                t.send_tablet_pad_button(self.id, pad.dev, time_usec, button, state)
            });
            if pad.tablet.is_some() {
                pad.pad_owner.button(&pad, time_usec, button, state);
            }
        }
    }

    pub fn tablet_event_pad_ring(
        self: &Rc<Self>,
        pad: TabletPadId,
        ring: u32,
        source: Option<TabletRingEventSource>,
        angle: Option<f64>,
        time_usec: u64,
    ) {
        if let Some(pad) = self.tablet.pads.get(&pad) {
            self.state.for_each_seat_tester(|t| {
                t.send_tablet_pad_ring(self.id, pad.dev, time_usec, ring, source, angle)
            });
            if pad.tablet.is_some() {
                if let Some(ring) = pad.rings.get(ring as usize) {
                    let node = self.keyboard_node.get();
                    node.node_on_tablet_pad_ring(&pad, ring, source, angle, time_usec);
                }
            }
        }
    }

    pub fn tablet_event_pad_strip(
        self: &Rc<Self>,
        pad: TabletPadId,
        strip: u32,
        source: Option<TabletStripEventSource>,
        position: Option<f64>,
        time_usec: u64,
    ) {
        if let Some(pad) = self.tablet.pads.get(&pad) {
            self.state.for_each_seat_tester(|t| {
                t.send_tablet_pad_strip(self.id, pad.dev, time_usec, strip, source, position)
            });
            if pad.tablet.is_some() {
                if let Some(strip) = pad.strips.get(strip as usize) {
                    let node = pad.node.get();
                    node.node_on_tablet_pad_strip(&pad, strip, source, position, time_usec);
                }
            }
        }
    }
}

impl TabletPad {
    fn for_each_pair(&self, n: &WlSurface, mut f: impl FnMut(&ZwpTabletV2, &ZwpTabletPadV2)) {
        let Some(tablet) = self.tablet.get() else {
            return;
        };
        self.seat.tablet_for_each_seat(n, |s| {
            let Some(tablet) = tablet.bindings.get(s) else {
                return;
            };
            let Some(pad) = self.bindings.get(s) else {
                return;
            };
            f(&tablet, &pad);
        })
    }

    fn for_each_entered(&self, n: &WlSurface, mut f: impl FnMut(&ZwpTabletPadV2)) {
        self.seat.tablet_for_each_seat(n, |s| {
            let Some(pad) = self.bindings.get(s) else {
                return;
            };
            if pad.entered.get() {
                f(&pad);
            }
        })
    }

    pub fn surface_enter(self: &Rc<Self>, n: &WlSurface) {
        let mut serial = n.client.pending_serial();
        let time = n.client.state.now_msec() as u32;
        self.for_each_pair(n, |tablet, pad| {
            pad.send_enter(serial.get(), &tablet, n);
            for group in &self.groups {
                let mode = group.mode.get();
                if let Some(group) = group.bindings.get(&pad.seat) {
                    group.send_mode_switch(time, serial.get(), mode);
                }
            }
        });
    }

    pub fn surface_leave(self: &Rc<Self>, n: &WlSurface) {
        let mut serial = n.client.pending_serial();
        self.for_each_entered(n, |pad| {
            pad.send_leave(serial.get(), n);
        });
    }

    pub fn surface_ring(
        self: &Rc<Self>,
        n: &WlSurface,
        ring: &Rc<TabletPadRing>,
        source: Option<TabletRingEventSource>,
        angle: Option<f64>,
        time_usec: u64,
    ) {
        let time = usec_to_msec(time_usec);
        self.seat.tablet_for_each_seat(n, |s| {
            if let Some(ring) = ring.bindings.get(&s) {
                if let Some(source) = source {
                    ring.send_source(source);
                }
                if let Some(angle) = angle {
                    ring.send_angle(Fixed::from_f64(angle));
                } else {
                    ring.send_stop();
                }
                ring.send_frame(time);
            }
        });
    }

    pub fn surface_strip(
        self: &Rc<Self>,
        n: &WlSurface,
        strip: &Rc<TabletPadStrip>,
        source: Option<TabletStripEventSource>,
        position: Option<f64>,
        time_usec: u64,
    ) {
        let time = usec_to_msec(time_usec);
        self.for_each_entered(n, |pad| {
            if let Some(strip) = strip.bindings.get(&pad.seat) {
                if let Some(source) = source {
                    strip.send_source(source);
                }
                if let Some(position) = position {
                    strip.send_position(normalizeu(position));
                } else {
                    strip.send_stop();
                }
                strip.send_frame(time);
            }
        });
    }

    pub fn surface_mode_switch(
        self: &Rc<Self>,
        n: &WlSurface,
        group: &Rc<TabletPadGroup>,
        time_usec: u64,
        mode: u32,
    ) {
        let time = usec_to_msec(time_usec);
        let mut serial = n.client.pending_serial();
        self.for_each_entered(n, |pad| {
            if let Some(group) = group.bindings.get(&pad.seat) {
                group.send_mode_switch(time, serial.get(), mode);
            }
        });
    }

    pub fn surface_button(
        self: &Rc<Self>,
        n: &WlSurface,
        time_usec: u64,
        button: u32,
        state: PadButtonState,
    ) {
        let time = usec_to_msec(time_usec);
        self.for_each_entered(n, |pad| {
            pad.send_button(time, button, state);
        })
    }
}
