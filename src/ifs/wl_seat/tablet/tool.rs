use {
    crate::{
        cursor::KnownCursor,
        fixed::Fixed,
        ifs::{
            wl_seat::{
                tablet::{
                    normalizei, normalizeu, zwp_tablet_tool_v2::ZwpTabletToolV2,
                    zwp_tablet_v2::ZwpTabletV2, TabletTool, TabletToolChanges, TabletToolId,
                    TabletToolInit, TabletToolOpt, TabletToolType, ToolButtonState,
                },
                WlSeatGlobal,
            },
            wl_surface::WlSurface,
        },
        rect::Rect,
        time::usec_to_msec,
        utils::{clonecell::CloneCell, hash_map_ext::HashMapExt},
    },
    std::{cell::Cell, rc::Rc},
};

impl WlSeatGlobal {
    pub fn tablet_handle_remove_tool(self: &Rc<Self>, time_usec: u64, id: TabletToolId) {
        let Some(tool) = self.tablet.tools.remove(&id) else {
            return;
        };
        self.state.for_each_seat_tester(|t| {
            t.send_tablet_tool_proximity_out(self.id, tool.tablet.dev, tool.id, time_usec)
        });
        tool.opt.tool.take();
        tool.cursor.detach();
        tool.tool_owner.destroy(&tool);
        for binding in tool.bindings.lock().drain_values() {
            binding.send_removed();
        }
        tool.tablet.tools.remove(&id);
    }

    pub fn tablet_handle_new_tool(self: &Rc<Self>, time_usec: u64, init: &TabletToolInit) {
        let Some(tablet) = self.tablet.tablets.get(&init.tablet_id) else {
            return;
        };
        let tool = Rc::new(TabletTool {
            id: init.id,
            opt: Default::default(),
            tablet,
            type_: init.type_,
            hardware_serial: init.hardware_serial,
            hardware_id_wacom: init.hardware_id_wacom,
            capabilities: init.capabilities.clone(),
            bindings: Default::default(),
            node: CloneCell::new(self.state.root.clone()),
            tool_owner: Default::default(),
            cursor: self.cursor_user_group.create_user(),
            down: Cell::new(false),
            pressure: Cell::new(0.0),
            distance: Cell::new(0.0),
            tilt_x: Cell::new(0.0),
            tilt_y: Cell::new(0.0),
            rotation: Cell::new(0.0),
            slider: Cell::new(0.0),
        });
        tool.opt.tool.set(Some(tool.clone()));
        tool.cursor.set_known(KnownCursor::Default);
        self.tablet.tools.set(init.id, tool.clone());
        self.state.for_each_seat_tester(|t| {
            t.send_tablet_tool_proximity_in(self.id, tool.tablet.dev, tool.id, time_usec)
        });
        self.tablet_for_each_seat_obj(|s| s.announce_tool(&tool));
    }

    pub fn tablet_event_tool_button(
        self: &Rc<Self>,
        id: TabletToolId,
        time_usec: u64,
        button: u32,
        state: ToolButtonState,
    ) {
        let Some(tool) = self.tablet.tools.get(&id) else {
            return;
        };
        self.state.for_each_seat_tester(|t| {
            t.send_tablet_tool_button(self.id, tool.tablet.dev, &tool, time_usec, button, state);
        });
        tool.cursor.activate();
        tool.tool_owner.button(&tool, time_usec, button, state);
    }

    pub fn tablet_event_tool_changes(
        self: &Rc<Self>,
        id: TabletToolId,
        time_usec: u64,
        rect: Rect,
        changes: &TabletToolChanges,
    ) {
        let Some(tool) = self.tablet.tools.get(&id) else {
            return;
        };
        self.state.for_each_seat_tester(|t| {
            t.send_tablet_tool_changes(self.id, tool.tablet.dev, &tool, time_usec, changes);
        });
        if let Some(val) = changes.down {
            tool.down.set(val);
        }
        if let Some(val) = changes.pressure {
            tool.pressure.set(val);
        }
        if let Some(val) = changes.distance {
            tool.distance.set(val);
        }
        if let Some(val) = changes.tilt {
            tool.tilt_x.set(val.x);
            tool.tilt_y.set(val.y);
        }
        if let Some(val) = changes.rotation {
            tool.rotation.set(val);
        }
        if let Some(val) = changes.slider {
            tool.slider.set(val);
        }
        if let Some(delta) = changes.pos {
            let (x, y) = match tool.type_ {
                TabletToolType::Mouse | TabletToolType::Lens => {
                    let (mut x, mut y) = tool.cursor.position();
                    x += Fixed::from_f64(delta.x.dx);
                    y += Fixed::from_f64(delta.y.dx);
                    (x, y)
                }
                TabletToolType::Pen
                | TabletToolType::Eraser
                | TabletToolType::Brush
                | TabletToolType::Pencil
                | TabletToolType::Airbrush
                | TabletToolType::Finger => {
                    let x = Fixed::from_f64(rect.x1() as f64 + (rect.width() as f64 * delta.x.x));
                    let y = Fixed::from_f64(rect.y1() as f64 + (rect.height() as f64 * delta.y.x));
                    (x, y)
                }
            };
            tool.cursor.set_position(x, y);
        }
        tool.cursor.activate();
        tool.tool_owner
            .apply_changes(&tool, time_usec, Some(changes));
    }
}

impl TabletTool {
    fn for_each_pair(&self, n: &WlSurface, mut f: impl FnMut(&ZwpTabletV2, &ZwpTabletToolV2)) {
        self.tablet.seat.tablet_for_each_seat(n, |s| {
            let Some(tablet) = self.tablet.bindings.get(s) else {
                return;
            };
            let Some(tool) = self.bindings.get(s) else {
                return;
            };
            f(&tablet, &tool);
        })
    }

    fn for_each_entered(&self, n: &WlSurface, mut f: impl FnMut(&ZwpTabletToolV2)) {
        self.tablet.seat.tablet_for_each_seat(n, |s| {
            let Some(tool) = self.bindings.get(s) else {
                return;
            };
            if !tool.entered.get() {
                return;
            }
            f(&tool);
        })
    }

    pub fn surface_leave(&self, n: &WlSurface, time_usec: u64) {
        let time = usec_to_msec(time_usec);
        self.for_each_entered(n, |t| {
            t.send_proximity_out();
            t.send_frame(time);
        })
    }

    pub fn surface_enter(&self, n: &WlSurface, time_usec: u64, x: Fixed, y: Fixed) {
        let time = usec_to_msec(time_usec);
        let mut serial = n.client.pending_serial();
        self.for_each_pair(n, |tablet, tool| {
            tool.send_proximity_in(serial.get(), tablet, n);
            tool.send_motion(x, y);
            tool.send_pressure(normalizeu(self.pressure.get()));
            tool.send_distance(normalizeu(self.distance.get()));
            tool.send_tilt(
                Fixed::from_f64(self.tilt_x.get()),
                Fixed::from_f64(self.tilt_y.get()),
            );
            tool.send_rotation(Fixed::from_f64(self.rotation.get()));
            tool.send_slider(normalizei(self.slider.get()));
            tool.send_frame(time);
        })
    }

    pub fn surface_button(
        &self,
        n: &WlSurface,
        time_usec: u64,
        button: u32,
        state: ToolButtonState,
    ) {
        let time = usec_to_msec(time_usec);
        let mut serial = n.client.pending_serial();
        self.for_each_entered(n, |tool| {
            tool.send_button(serial.get(), button, state);
            tool.send_frame(time);
        });
        if state == ToolButtonState::Pressed {
            if let Some(node) = n.get_focus_node(self.tablet.seat.id) {
                self.tablet.seat.focus_node_with_serial(node, serial.get());
            }
        }
    }

    pub fn surface_apply_changes(
        &self,
        n: &WlSurface,
        time_usec: u64,
        changes: Option<&TabletToolChanges>,
        x: Fixed,
        y: Fixed,
    ) {
        let mut serial = n.client.pending_serial();
        let time = usec_to_msec(time_usec);
        self.for_each_entered(n, |tool| {
            if let Some(changes) = changes {
                if let Some(val) = changes.down {
                    match val {
                        false => tool.send_up(),
                        true => tool.send_down(serial.get()),
                    }
                }
                if let Some(val) = changes.pressure {
                    tool.send_pressure(normalizeu(val));
                }
                if let Some(val) = changes.distance {
                    tool.send_distance(normalizeu(val));
                }
                if let Some(val) = changes.tilt {
                    tool.send_tilt(Fixed::from_f64(val.x), Fixed::from_f64(val.y));
                }
                if let Some(val) = changes.rotation {
                    tool.send_rotation(Fixed::from_f64(val));
                }
                if let Some(val) = changes.slider {
                    tool.send_slider(normalizei(val));
                }
                if let Some(val) = changes.wheel {
                    tool.send_wheel(Fixed::from_f64(val.degrees), val.clicks);
                }
            }
            tool.send_motion(x, y);
            tool.send_frame(time);
        });
        if let Some(changes) = changes {
            if changes.down == Some(true) {
                if let Some(node) = n.get_focus_node(self.tablet.seat.id) {
                    self.tablet.seat.focus_node_with_serial(node, serial.get());
                }
            }
        }
    }
}

impl TabletToolOpt {
    pub fn get(&self) -> Option<Rc<TabletTool>> {
        self.tool.get()
    }
}
