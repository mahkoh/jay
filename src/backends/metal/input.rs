use {
    crate::{
        backend::{AxisSource, InputEvent, KeyState, ScrollAxis},
        backends::metal::MetalBackend,
        fixed::Fixed,
        ifs::wl_seat::tablet::{
            PadButtonState, TabletRingEventSource, TabletStripEventSource, TabletTool2dChange,
            TabletToolCapability, TabletToolChanges, TabletToolId, TabletToolInit,
            TabletToolPositionChange, TabletToolType, TabletToolWheelChange, ToolButtonState,
        },
        libinput::{
            consts::{
                LIBINPUT_BUTTON_STATE_PRESSED, LIBINPUT_BUTTON_STATE_RELEASED,
                LIBINPUT_KEY_STATE_PRESSED, LIBINPUT_POINTER_AXIS_SCROLL_HORIZONTAL,
                LIBINPUT_POINTER_AXIS_SCROLL_VERTICAL, LIBINPUT_SWITCH_LID,
                LIBINPUT_SWITCH_STATE_OFF, LIBINPUT_SWITCH_STATE_ON, LIBINPUT_SWITCH_TABLET_MODE,
                LIBINPUT_TABLET_PAD_RING_SOURCE_FINGER, LIBINPUT_TABLET_PAD_STRIP_SOURCE_FINGER,
                LIBINPUT_TABLET_TOOL_PROXIMITY_STATE_IN, LIBINPUT_TABLET_TOOL_TIP_DOWN,
                LIBINPUT_TABLET_TOOL_TIP_UP, LIBINPUT_TABLET_TOOL_TYPE_AIRBRUSH,
                LIBINPUT_TABLET_TOOL_TYPE_BRUSH, LIBINPUT_TABLET_TOOL_TYPE_ERASER,
                LIBINPUT_TABLET_TOOL_TYPE_LENS, LIBINPUT_TABLET_TOOL_TYPE_MOUSE,
                LIBINPUT_TABLET_TOOL_TYPE_PEN, LIBINPUT_TABLET_TOOL_TYPE_PENCIL,
            },
            event::{LibInputEvent, LibInputEventTabletTool},
        },
        utils::{bitflags::BitflagsExt, errorfmt::ErrorFmt},
    },
    jay_config::input::SwitchEvent,
    std::rc::Rc,
    uapi::c,
};

macro_rules! unpack {
    ($slf:expr, $ev:expr) => {{
        let slot = match $ev.device().slot() {
            Some(s) => s,
            _ => return,
        };
        let data = match $slf
            .device_holder
            .input_devices
            .borrow_mut()
            .get(slot)
            .cloned()
            .and_then(|v| v)
        {
            Some(d) => d,
            _ => return,
        };
        data
    }};
    ($slf:expr, $ev:expr, $conv:ident) => {{
        let event = match $ev.$conv() {
            Some(e) => e,
            _ => return,
        };
        let data = unpack!($slf, $ev);
        (event, data)
    }};
}

impl MetalBackend {
    pub async fn handle_libinput_events(self: Rc<Self>) {
        loop {
            match self.state.ring.readable(&self.libinput_fd).await {
                Err(e) => {
                    log::error!(
                        "Cannot wait for libinput fd to become readable: {}",
                        ErrorFmt(e)
                    );
                    break;
                }
                Ok(n) if n.intersects(c::POLLERR | c::POLLHUP) => {
                    log::error!("libinput fd fd is in an error state");
                    break;
                }
                _ => {}
            }
            if let Err(e) = self.libinput.dispatch() {
                log::error!("Could not dispatch libinput events: {}", ErrorFmt(e));
                break;
            }
            while let Some(event) = self.libinput.event() {
                self.handle_event(event);
            }
        }
        log::error!("Libinput task exited. Future input events will be ignored.");
    }

    fn handle_event(self: &Rc<Self>, event: LibInputEvent) {
        use crate::libinput::consts as c;

        match event.ty() {
            c::LIBINPUT_EVENT_DEVICE_ADDED => self.handle_device_added(event),
            c::LIBINPUT_EVENT_DEVICE_REMOVED => self.handle_li_device_removed(event),
            c::LIBINPUT_EVENT_KEYBOARD_KEY => self.handle_keyboard_key(event),
            c::LIBINPUT_EVENT_POINTER_MOTION => self.handle_pointer_motion(event),
            c::LIBINPUT_EVENT_POINTER_BUTTON => self.handle_pointer_button(event),
            c::LIBINPUT_EVENT_POINTER_SCROLL_WHEEL => {
                self.handle_pointer_axis(event, AxisSource::Wheel)
            }
            c::LIBINPUT_EVENT_POINTER_SCROLL_FINGER => {
                self.handle_pointer_axis(event, AxisSource::Finger)
            }
            c::LIBINPUT_EVENT_POINTER_SCROLL_CONTINUOUS => {
                self.handle_pointer_axis(event, AxisSource::Continuous)
            }
            c::LIBINPUT_EVENT_GESTURE_SWIPE_BEGIN => self.handle_gesture_swipe_begin(event),
            c::LIBINPUT_EVENT_GESTURE_SWIPE_UPDATE => self.handle_gesture_swipe_update(event),
            c::LIBINPUT_EVENT_GESTURE_SWIPE_END => self.handle_gesture_swipe_end(event),
            c::LIBINPUT_EVENT_GESTURE_PINCH_BEGIN => self.handle_gesture_pinch_begin(event),
            c::LIBINPUT_EVENT_GESTURE_PINCH_UPDATE => self.handle_gesture_pinch_update(event),
            c::LIBINPUT_EVENT_GESTURE_PINCH_END => self.handle_gesture_pinch_end(event),
            c::LIBINPUT_EVENT_GESTURE_HOLD_BEGIN => self.handle_gesture_hold_begin(event),
            c::LIBINPUT_EVENT_GESTURE_HOLD_END => self.handle_gesture_hold_end(event),
            c::LIBINPUT_EVENT_SWITCH_TOGGLE => self.handle_switch_toggle(event),
            c::LIBINPUT_EVENT_TABLET_TOOL_PROXIMITY => self.handle_tablet_tool_proximity(event),
            c::LIBINPUT_EVENT_TABLET_TOOL_AXIS => self.handle_tablet_tool_axis(event),
            c::LIBINPUT_EVENT_TABLET_TOOL_BUTTON => self.handle_tablet_tool_button(event),
            c::LIBINPUT_EVENT_TABLET_TOOL_TIP => self.handle_tablet_tool_tip(event),
            c::LIBINPUT_EVENT_TABLET_PAD_BUTTON => self.handle_tablet_pad_button(event),
            c::LIBINPUT_EVENT_TABLET_PAD_RING => self.handle_tablet_pad_ring(event),
            c::LIBINPUT_EVENT_TABLET_PAD_STRIP => self.handle_tablet_pad_strip(event),
            _ => {}
        }
    }

    fn handle_device_added(self: &Rc<Self>, _event: LibInputEvent) {
        // let dev = unpack!(self, event);
    }

    fn handle_li_device_removed(self: &Rc<Self>, event: LibInputEvent) {
        let dev = unpack!(self, event);
        dev.inputdev.set(None);
        event.device().unset_slot();
    }

    fn handle_keyboard_key(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, keyboard_event);
        let state = if event.key_state() == LIBINPUT_KEY_STATE_PRESSED {
            if dev.pressed_keys.insert(event.key(), ()).is_some() {
                return;
            }
            KeyState::Pressed
        } else {
            if dev.pressed_keys.remove(&event.key()).is_none() {
                return;
            }
            KeyState::Released
        };
        dev.event(InputEvent::Key {
            time_usec: event.time_usec(),
            key: event.key(),
            state,
        });
    }

    fn handle_pointer_axis(self: &Rc<Self>, event: LibInputEvent, source: AxisSource) {
        let (event, dev) = unpack!(self, event, pointer_event);
        let axes = [
            (
                LIBINPUT_POINTER_AXIS_SCROLL_HORIZONTAL,
                ScrollAxis::Horizontal,
            ),
            (LIBINPUT_POINTER_AXIS_SCROLL_VERTICAL, ScrollAxis::Vertical),
        ];
        dev.event(InputEvent::AxisSource { source });
        for (pointer_axis, axis) in axes {
            if !event.has_axis(pointer_axis) {
                continue;
            }
            let scroll = match source {
                AxisSource::Wheel => event.scroll_value_v120(pointer_axis),
                _ => event.scroll_value(pointer_axis),
            };
            let ie = if scroll == 0.0 {
                InputEvent::AxisStop { axis }
            } else if source == AxisSource::Wheel {
                InputEvent::Axis120 {
                    dist: scroll as _,
                    axis,
                    inverted: dev
                        .effective
                        .natural_scrolling_enabled
                        .get()
                        .unwrap_or_default(),
                }
            } else {
                InputEvent::AxisPx {
                    dist: Fixed::from_f64(scroll),
                    axis,
                    inverted: dev
                        .effective
                        .natural_scrolling_enabled
                        .get()
                        .unwrap_or_default(),
                }
            };
            dev.event(ie);
        }
        dev.event(InputEvent::AxisFrame {
            time_usec: event.time_usec(),
        });
    }

    fn handle_pointer_button(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, pointer_event);
        let state = if event.button_state() == LIBINPUT_BUTTON_STATE_PRESSED {
            if dev.pressed_buttons.insert(event.button(), ()).is_some() {
                return;
            }
            KeyState::Pressed
        } else {
            if dev.pressed_buttons.remove(&event.button()).is_none() {
                return;
            }
            KeyState::Released
        };
        dev.event(InputEvent::Button {
            time_usec: event.time_usec(),
            button: event.button(),
            state,
        });
    }

    fn handle_pointer_motion(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, pointer_event);
        let mut dx = event.dx();
        let mut dy = event.dy();
        let mut dx_unaccelerated = event.dx_unaccelerated();
        let mut dy_unaccelerated = event.dy_unaccelerated();
        if let Some(matrix) = dev.transform_matrix.get() {
            (dx, dy) = (
                matrix[0][0] * dx + matrix[0][1] * dy,
                matrix[1][0] * dx + matrix[1][1] * dy,
            );
            (dx_unaccelerated, dy_unaccelerated) = (
                matrix[0][0] * dx_unaccelerated + matrix[0][1] * dy_unaccelerated,
                matrix[1][0] * dx_unaccelerated + matrix[1][1] * dy_unaccelerated,
            );
        }
        dev.event(InputEvent::Motion {
            time_usec: event.time_usec(),
            dx: Fixed::from_f64(dx),
            dy: Fixed::from_f64(dy),
            dx_unaccelerated: Fixed::from_f64(dx_unaccelerated),
            dy_unaccelerated: Fixed::from_f64(dy_unaccelerated),
        });
    }

    fn handle_gesture_swipe_begin(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, gesture_event);
        dev.event(InputEvent::SwipeBegin {
            time_usec: event.time_usec(),
            finger_count: event.finger_count(),
        });
    }

    fn handle_gesture_swipe_update(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, gesture_event);
        dev.event(InputEvent::SwipeUpdate {
            time_usec: event.time_usec(),
            dx: Fixed::from_f64(event.dx()),
            dy: Fixed::from_f64(event.dy()),
            dx_unaccelerated: Fixed::from_f64(event.dx_unaccelerated()),
            dy_unaccelerated: Fixed::from_f64(event.dy_unaccelerated()),
        });
    }

    fn handle_gesture_swipe_end(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, gesture_event);
        dev.event(InputEvent::SwipeEnd {
            time_usec: event.time_usec(),
            cancelled: event.cancelled(),
        });
    }

    fn handle_gesture_pinch_begin(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, gesture_event);
        dev.event(InputEvent::PinchBegin {
            time_usec: event.time_usec(),
            finger_count: event.finger_count(),
        });
    }

    fn handle_gesture_pinch_update(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, gesture_event);
        dev.event(InputEvent::PinchUpdate {
            time_usec: event.time_usec(),
            dx: Fixed::from_f64(event.dx()),
            dy: Fixed::from_f64(event.dy()),
            dx_unaccelerated: Fixed::from_f64(event.dx_unaccelerated()),
            dy_unaccelerated: Fixed::from_f64(event.dy_unaccelerated()),
            scale: Fixed::from_f64(event.scale()),
            rotation: Fixed::from_f64(event.angle_delta()),
        });
    }

    fn handle_gesture_pinch_end(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, gesture_event);
        dev.event(InputEvent::PinchEnd {
            time_usec: event.time_usec(),
            cancelled: event.cancelled(),
        });
    }

    fn handle_gesture_hold_begin(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, gesture_event);
        dev.event(InputEvent::HoldBegin {
            time_usec: event.time_usec(),
            finger_count: event.finger_count(),
        });
    }

    fn handle_gesture_hold_end(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, gesture_event);
        dev.event(InputEvent::HoldEnd {
            time_usec: event.time_usec(),
            cancelled: event.cancelled(),
        });
    }

    fn handle_switch_toggle(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, switch_event);
        let switch_event = match (event.switch(), event.switch_state()) {
            (LIBINPUT_SWITCH_LID, LIBINPUT_SWITCH_STATE_OFF) => SwitchEvent::LidOpened,
            (LIBINPUT_SWITCH_LID, LIBINPUT_SWITCH_STATE_ON) => SwitchEvent::LidClosed,
            (LIBINPUT_SWITCH_TABLET_MODE, LIBINPUT_SWITCH_STATE_OFF) => {
                SwitchEvent::ConvertedToLaptop
            }
            (LIBINPUT_SWITCH_TABLET_MODE, LIBINPUT_SWITCH_STATE_ON) => {
                SwitchEvent::ConvertedToTablet
            }
            _ => return,
        };
        dev.event(InputEvent::SwitchEvent {
            time_usec: event.time_usec(),
            event: switch_event,
        });
    }

    fn get_tool_id(&self, event: &LibInputEventTabletTool) -> TabletToolId {
        let tool = event.tool();
        let mut user_data = tool.user_data();
        if user_data == 0 {
            user_data = self.state.tablet_tool_ids.next().raw();
            tool.set_user_data(user_data);
        }
        TabletToolId::from_raw(user_data)
    }

    fn build_tablet_tool_changed(
        &self,
        event: &LibInputEventTabletTool,
        down: Option<bool>,
    ) -> InputEvent {
        let mut changes = Box::<TabletToolChanges>::default();
        changes.down = down;
        if event.x_has_changed() || event.y_has_changed() {
            changes.pos = Some(TabletTool2dChange {
                x: TabletToolPositionChange {
                    x: event.x_transformed(1),
                    dx: event.dx(),
                },
                y: TabletToolPositionChange {
                    x: event.y_transformed(1),
                    dx: event.dy(),
                },
            })
        }
        if event.pressure_has_changed() {
            changes.pressure = Some(event.pressure());
        }
        if event.distance_has_changed() {
            changes.distance = Some(event.distance());
        }
        if event.tilt_x_has_changed() || event.tilt_y_has_changed() {
            changes.tilt = Some(TabletTool2dChange {
                x: event.tilt_x(),
                y: event.tilt_y(),
            });
        }
        if event.rotation_has_changed() {
            changes.rotation = Some(event.rotation());
        }
        if event.slider_has_changed() {
            changes.slider = Some(event.slider_position());
        }
        if event.wheel_has_changed() {
            changes.wheel = Some(TabletToolWheelChange {
                degrees: event.wheel_delta(),
                clicks: event.wheel_delta_discrete(),
            });
        }
        InputEvent::TabletToolChanged {
            time_usec: event.time_usec(),
            id: self.get_tool_id(event),
            changes,
        }
    }

    fn handle_tablet_tool_proximity(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, tablet_tool_event);
        let id = self.get_tool_id(&event);
        if event.proximity_state() == LIBINPUT_TABLET_TOOL_PROXIMITY_STATE_IN {
            let Some(tablet_id) = dev.tablet_id.get() else {
                return;
            };
            let tool = event.tool();
            dev.event(InputEvent::TabletToolAdded {
                time_usec: event.time_usec(),
                init: Box::new(TabletToolInit {
                    tablet_id,
                    id,
                    type_: match tool.type_() {
                        LIBINPUT_TABLET_TOOL_TYPE_PEN => TabletToolType::Pen,
                        LIBINPUT_TABLET_TOOL_TYPE_ERASER => TabletToolType::Eraser,
                        LIBINPUT_TABLET_TOOL_TYPE_BRUSH => TabletToolType::Brush,
                        LIBINPUT_TABLET_TOOL_TYPE_PENCIL => TabletToolType::Pencil,
                        LIBINPUT_TABLET_TOOL_TYPE_AIRBRUSH => TabletToolType::Airbrush,
                        LIBINPUT_TABLET_TOOL_TYPE_MOUSE => TabletToolType::Mouse,
                        LIBINPUT_TABLET_TOOL_TYPE_LENS => TabletToolType::Lens,
                        _ => return,
                    },
                    hardware_serial: tool.serial(),
                    hardware_id_wacom: tool.tool_id(),
                    capabilities: {
                        let mut caps = vec![];
                        macro_rules! add_cap {
                            ($f:ident, $cap:ident) => {
                                if tool.$f() {
                                    caps.push(TabletToolCapability::$cap);
                                }
                            };
                        }
                        add_cap!(has_tilt, Tilt);
                        add_cap!(has_pressure, Pressure);
                        add_cap!(has_distance, Distance);
                        add_cap!(has_rotation, Rotation);
                        add_cap!(has_slider, Slider);
                        add_cap!(has_wheel, Wheel);
                        caps
                    },
                }),
            });
            dev.event(self.build_tablet_tool_changed(&event, None));
        } else {
            dev.event(InputEvent::TabletToolRemoved {
                time_usec: event.time_usec(),
                id,
            });
        }
    }

    fn handle_tablet_tool_tip(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, tablet_tool_event);
        let down = match event.tip_state() {
            LIBINPUT_TABLET_TOOL_TIP_UP => false,
            LIBINPUT_TABLET_TOOL_TIP_DOWN => true,
            _ => return,
        };
        dev.event(self.build_tablet_tool_changed(&event, Some(down)));
    }

    fn handle_tablet_tool_axis(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, tablet_tool_event);
        dev.event(self.build_tablet_tool_changed(&event, None));
    }

    fn handle_tablet_tool_button(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, tablet_tool_event);
        dev.event(InputEvent::TabletToolButton {
            time_usec: event.time_usec(),
            id: self.get_tool_id(&event),
            button: event.button(),
            state: match event.button_state() {
                LIBINPUT_BUTTON_STATE_RELEASED => ToolButtonState::Released,
                LIBINPUT_BUTTON_STATE_PRESSED => ToolButtonState::Pressed,
                _ => return,
            },
        });
    }

    fn handle_tablet_pad_button(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, tablet_pad_event);
        let id = match dev.tablet_pad_id.get() {
            None => return,
            Some(id) => id,
        };
        let state = match event.button_state() {
            LIBINPUT_BUTTON_STATE_RELEASED => PadButtonState::Released,
            LIBINPUT_BUTTON_STATE_PRESSED => PadButtonState::Pressed,
            _ => return,
        };
        dev.event(InputEvent::TabletPadModeSwitch {
            time_usec: event.time_usec(),
            pad: id,
            group: event.mode_group().index(),
            mode: event.mode(),
        });
        dev.event(InputEvent::TabletPadButton {
            time_usec: event.time_usec(),
            id,
            button: event.button_number(),
            state,
        });
    }

    fn handle_tablet_pad_ring(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, tablet_pad_event);
        dev.event(InputEvent::TabletPadRing {
            time_usec: event.time_usec(),
            pad: match dev.tablet_pad_id.get() {
                None => return,
                Some(id) => id,
            },
            ring: event.ring_number(),
            source: match event.ring_source() {
                LIBINPUT_TABLET_PAD_RING_SOURCE_FINGER => Some(TabletRingEventSource::Finger),
                _ => None,
            },
            angle: match event.ring_position() {
                n if n == -1.0 => None,
                n => Some(n),
            },
        });
    }

    fn handle_tablet_pad_strip(self: &Rc<Self>, event: LibInputEvent) {
        let (event, dev) = unpack!(self, event, tablet_pad_event);
        dev.event(InputEvent::TabletPadStrip {
            time_usec: event.time_usec(),
            pad: match dev.tablet_pad_id.get() {
                None => return,
                Some(id) => id,
            },
            strip: event.strip_number(),
            source: match event.strip_source() {
                LIBINPUT_TABLET_PAD_STRIP_SOURCE_FINGER => Some(TabletStripEventSource::Finger),
                _ => None,
            },
            position: match event.strip_position() {
                n if n == -1.0 => None,
                n => Some(n),
            },
        });
    }
}
