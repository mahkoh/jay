use {
    crate::{
        backend::{
            BackendConnectorState, BackendConnectorStateSerial, Connector, ConnectorEvent,
            ConnectorId, MonitorInfo,
        },
        format::XRGB8888,
        globals::GlobalName,
        ifs::{
            head_management::{HeadManagers, HeadState},
            jay_tray_v1::JayTrayV1Global,
            wl_output::{PersistentOutputState, WlOutputGlobal},
        },
        output_schedule::OutputSchedule,
        state::{ConnectorData, OutputData, State},
        tree::{OutputNode, WsMoveConfig, move_ws_to_output},
        utils::{
            asyncevent::AsyncEvent, clonecell::CloneCell, hash_map_ext::HashMapExt, rc_eq::RcEq,
        },
    },
    jay_config::video::Transform,
    std::{cell::Cell, collections::VecDeque, rc::Rc},
};

pub fn handle(state: &Rc<State>, connector: &Rc<dyn Connector>) {
    let mut drm_dev = None;
    if let Some(dev_id) = connector.drm_dev() {
        drm_dev = match state.drm_devs.get(&dev_id) {
            Some(dev) => Some(dev),
            _ => panic!("connector's drm device does not exist"),
        };
    }
    let backend_state = BackendConnectorState {
        serial: BackendConnectorStateSerial::from_raw(0),
        enabled: true,
        active: false,
        mode: Default::default(),
        non_desktop_override: None,
        vrr: false,
        tearing: false,
        format: XRGB8888,
        color_space: Default::default(),
        transfer_function: Default::default(),
    };
    let id = connector.id();
    let name = Rc::new(connector.kernel_id().to_string());
    let head_state = HeadState {
        name: RcEq(name.clone()),
        position: (0, 0),
        size: (0, 0),
        transform: Transform::None,
        scale: Default::default(),
        wl_output: None,
        connector_enabled: backend_state.enabled,
        in_compositor_space: false,
        mode: Default::default(),
        monitor_info: None,
    };
    let data = Rc::new(ConnectorData {
        id,
        connector: connector.clone(),
        handler: Default::default(),
        connected: Cell::new(false),
        name,
        drm_dev: drm_dev.clone(),
        async_event: Rc::new(AsyncEvent::default()),
        damaged: Cell::new(false),
        damage: Default::default(),
        needs_vblank_emulation: Cell::new(false),
        damage_intersect: Default::default(),
        state: Cell::new(backend_state),
        head_managers: HeadManagers::new(state.head_names.next(), head_state),
    });
    if let Some(dev) = drm_dev {
        dev.connectors.set(id, data.clone());
    }
    let oh = ConnectorHandler {
        id,
        state: state.clone(),
        data: data.clone(),
    };
    let future = state.eng.spawn("connector handler", oh.handle());
    data.handler.set(Some(future));
    for mgr in state.head_managers.lock().values() {
        mgr.announce(&data);
    }
    if state.connectors.set(id, data).is_some() {
        panic!("Connector id has been reused");
    }
}

struct ConnectorHandler {
    id: ConnectorId,
    state: Rc<State>,
    data: Rc<ConnectorData>,
}

impl ConnectorHandler {
    async fn handle(self) {
        {
            let ae = self.data.async_event.clone();
            self.data.connector.on_change(Rc::new(move || ae.trigger()));
        }
        if let Some(config) = self.state.config.get() {
            config.new_connector(self.id);
        }
        'outer: loop {
            while let Some(event) = self.data.connector.event() {
                match event {
                    ConnectorEvent::Removed => break 'outer,
                    ConnectorEvent::Connected(mi) => self.handle_connected(mi).await,
                    _ => unreachable!(),
                }
            }
            self.data.async_event.triggered().await;
        }
        if let Some(dev) = &self.data.drm_dev {
            dev.connectors.remove(&self.id);
        }
        if let Some(config) = self.state.config.get() {
            config.del_connector(self.id);
        }
        self.data.handler.set(None);
        self.state.connectors.remove(&self.id);
        self.data.head_managers.handle_removed();
    }

    async fn handle_connected(&self, info: MonitorInfo) {
        log::info!("Connector {} connected", self.data.connector.kernel_id());
        self.data.connected.set(true);
        self.data.set_state(&self.state, info.state);
        let name = self.state.globals.name();
        if info.non_desktop {
            self.handle_non_desktop_connected(info).await;
        } else {
            self.handle_desktop_connected(info, name).await;
        }
        self.data.connected.set(false);
        self.data.head_managers.handle_output_disconnected();
        log::info!("Connector {} disconnected", self.data.connector.kernel_id());
    }

    async fn handle_desktop_connected(&self, info: MonitorInfo, name: GlobalName) {
        let output_id = info.output_id.clone();
        let desired_state = match self.state.persistent_output_states.get(&output_id) {
            Some(ds) => ds,
            _ => {
                let x1 = self
                    .state
                    .root
                    .outputs
                    .lock()
                    .values()
                    .map(|o| o.global.pos.get().x2())
                    .max()
                    .unwrap_or(0);
                let ds = Rc::new(PersistentOutputState {
                    transform: Default::default(),
                    scale: Default::default(),
                    pos: Cell::new((x1, 0)),
                    vrr_mode: Cell::new(self.state.default_vrr_mode.get()),
                    vrr_cursor_hz: Cell::new(self.state.default_vrr_cursor_hz.get()),
                    tearing_mode: Cell::new(self.state.default_tearing_mode.get()),
                    brightness: Cell::new(None),
                });
                self.state
                    .persistent_output_states
                    .set(output_id.clone(), ds.clone());
                ds
            }
        };
        let global = Rc::new(WlOutputGlobal::new(
            name,
            &self.state,
            &self.data,
            info.modes.clone(),
            info.width_mm,
            info.height_mm,
            &output_id,
            &desired_state,
            info.transfer_functions.clone(),
            info.color_spaces.clone(),
            info.primaries,
            info.luminance,
        ));
        let schedule = Rc::new(OutputSchedule::new(
            &self.state.ring,
            &self.state.eng,
            &self.data,
            &desired_state,
        ));
        let _schedule = self
            .state
            .eng
            .spawn("output schedule", schedule.clone().drive());
        let tray = Rc::new(JayTrayV1Global {
            name: self.state.globals.name(),
            output: global.opt.clone(),
        });
        let on = Rc::new(OutputNode {
            id: self.state.node_ids.next(),
            workspaces: Default::default(),
            workspace: CloneCell::new(None),
            seat_state: Default::default(),
            global: global.clone(),
            layers: Default::default(),
            exclusive_zones: Default::default(),
            workspace_rect: Default::default(),
            non_exclusive_rect: Default::default(),
            non_exclusive_rect_rel: Default::default(),
            render_data: Default::default(),
            state: self.state.clone(),
            is_dummy: false,
            status: self.state.status.clone(),
            scroll: Default::default(),
            pointer_positions: Default::default(),
            pointer_down: Default::default(),
            lock_surface: Default::default(),
            hardware_cursor: Default::default(),
            jay_outputs: Default::default(),
            screencasts: Default::default(),
            update_render_data_scheduled: Cell::new(false),
            hardware_cursor_needs_render: Cell::new(false),
            screencopies: Default::default(),
            title_visible: Default::default(),
            schedule,
            latch_event: Default::default(),
            vblank_event: Default::default(),
            presentation_event: Default::default(),
            render_margin_ns: Default::default(),
            flip_margin_ns: Default::default(),
            ext_copy_sessions: Default::default(),
            before_latch_event: Default::default(),
            tray_start_rel: Default::default(),
            tray_items: Default::default(),
            ext_workspace_groups: Default::default(),
            pinned: Default::default(),
        });
        on.update_visible();
        on.update_rects();
        self.state
            .add_output_scale(on.global.persistent.scale.get());
        let output_data = Rc::new(OutputData {
            connector: self.data.clone(),
            monitor_info: Rc::new(info),
            node: Some(on.clone()),
            lease_connectors: Default::default(),
        });
        self.state.outputs.set(self.id, output_data.clone());
        on.schedule_update_render_data();
        self.state.root.outputs.set(self.id, on.clone());
        self.state.output_extents_changed();
        global.opt.node.set(Some(on.clone()));
        global.opt.global.set(Some(global.clone()));
        let mut ws_to_move = VecDeque::new();
        if self.state.root.outputs.len() == 1 {
            for seat in self.state.globals.seats.lock().values() {
                seat.cursor_group().first_output_connected(&on);
            }
            let dummy = self.state.dummy_output.get().unwrap();
            for ws in dummy.workspaces.iter() {
                if ws.is_dummy {
                    continue;
                }
                ws_to_move.push_back(ws);
            }
        }
        for source in self.state.root.outputs.lock().values() {
            if source.id == on.id {
                continue;
            }
            for ws in source.workspaces.iter() {
                if ws.is_dummy {
                    continue;
                }
                if ws.desired_output.get() == global.output_id {
                    ws_to_move.push_back(ws.clone());
                }
            }
        }
        while let Some(ws) = ws_to_move.pop_front() {
            let make_visible = (ws.visible_on_desired_output.get()
                && ws.desired_output.get() == output_id)
                || ws_to_move.is_empty();
            let config = WsMoveConfig {
                make_visible_always: false,
                make_visible_if_empty: make_visible,
                source_is_destroyed: false,
                before: None,
            };
            move_ws_to_output(&ws, &on, config);
        }
        if let Some(config) = self.state.config.get() {
            config.connector_connected(self.id);
        }
        self.state.add_global(&global);
        self.state.add_global(&tray);
        self.state.tree_changed();
        on.update_presentation_type();
        self.state.workspace_managers.announce_output(&on);
        self.data
            .head_managers
            .handle_output_connected(&output_data);
        'outer: loop {
            while let Some(event) = self.data.connector.event() {
                match event {
                    ConnectorEvent::Disconnected => break 'outer,
                    ConnectorEvent::HardwareCursor(hc) => {
                        on.schedule.set_hardware_cursor(&hc);
                        on.hardware_cursor.set(hc);
                        self.state.refresh_hardware_cursors();
                    }
                    ConnectorEvent::FormatsChanged(formats) => {
                        on.global.formats.set(formats);
                    }
                    ConnectorEvent::State(state) => {
                        self.data.set_state(&self.state, state);
                    }
                    ev => unreachable!("received unexpected event {:?}", ev),
                }
            }
            self.data.async_event.triggered().await;
        }
        if let Some(config) = self.state.config.get() {
            config.connector_disconnected(self.id);
        }
        global.clear();
        for jo in on.jay_outputs.lock().drain_values() {
            jo.send_destroyed();
        }
        let screencasts: Vec<_> = on.screencasts.lock().values().cloned().collect();
        for sc in screencasts {
            sc.do_destroy();
        }
        for sc in on.screencopies.lock().drain_values() {
            sc.send_failed();
        }
        for sc in on.ext_copy_sessions.lock().drain_values() {
            sc.stop();
        }
        global.destroyed.set(true);
        self.state.root.outputs.remove(&self.id);
        self.state.output_extents_changed();
        self.state.outputs.remove(&self.id);
        on.lock_surface.take();
        {
            let mut surfaces = vec![];
            for layer in &on.layers {
                surfaces.extend(layer.iter());
            }
            for surface in surfaces {
                surface.destroy_node();
                surface.send_closed();
            }
        }
        let target = match self.state.root.outputs.lock().values().next() {
            Some(o) => o.clone(),
            _ => self.state.dummy_output.get().unwrap(),
        };
        for ws in on.workspaces.iter() {
            if ws.desired_output.get() == output_id {
                ws.visible_on_desired_output.set(ws.visible.get());
            }
            let config = WsMoveConfig {
                make_visible_always: false,
                make_visible_if_empty: ws.visible.get(),
                source_is_destroyed: true,
                before: None,
            };
            move_ws_to_output(&ws, &target, config);
        }
        for group in on.ext_workspace_groups.lock().drain_values() {
            group.handle_destroyed();
        }
        for seat in self.state.globals.seats.lock().values() {
            seat.cursor_group().output_disconnected(&on, &target);
        }
        for item in on.tray_items.iter() {
            item.destroy_node();
        }
        self.state
            .remove_output_scale(on.global.persistent.scale.get());
        let _ = self.state.remove_global(&global);
        let _ = self.state.remove_global(&tray);
        self.state.tree_changed();
        self.state.damage(self.state.root.extents.get());
    }

    async fn handle_non_desktop_connected(&self, monitor_info: MonitorInfo) {
        let output_data = Rc::new(OutputData {
            connector: self.data.clone(),
            monitor_info: Rc::new(monitor_info),
            node: None,
            lease_connectors: Default::default(),
        });
        self.state.outputs.set(self.id, output_data.clone());
        let advertise = || {
            if let Some(dev) = &self.data.drm_dev {
                for binding in dev.lease_global.bindings.lock().values() {
                    binding.create_connector(&output_data);
                    binding.send_done();
                }
            }
        };
        let withdraw = || {
            for con in output_data.lease_connectors.lock().drain_values() {
                con.send_withdrawn();
                if !con.device.destroyed.get() {
                    con.device.send_done();
                }
            }
        };
        advertise();
        if let Some(config) = self.state.config.get() {
            config.connector_connected(self.id);
        }
        self.data
            .head_managers
            .handle_output_connected(&output_data);
        'outer: loop {
            while let Some(event) = self.data.connector.event() {
                match event {
                    ConnectorEvent::Disconnected => break 'outer,
                    ConnectorEvent::Available => advertise(),
                    ConnectorEvent::Unavailable => withdraw(),
                    ev => unreachable!("received unexpected event {:?}", ev),
                }
            }
            self.data.async_event.triggered().await;
        }
        withdraw();
        self.state.outputs.remove(&self.id);
        if let Some(config) = self.state.config.get() {
            config.connector_disconnected(self.id);
        }
    }
}
