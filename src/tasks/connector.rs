use {
    crate::{
        backend::{Connector, ConnectorEvent, ConnectorId, MonitorInfo},
        ifs::wl_output::WlOutputGlobal,
        state::{ConnectorData, OutputData, State},
        tree::{OutputNode, OutputRenderData},
        utils::{asyncevent::AsyncEvent, clonecell::CloneCell},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

pub fn handle(state: &Rc<State>, connector: &Rc<dyn Connector>) {
    let mut drm_dev = None;
    if let Some(dev_id) = connector.drm_dev() {
        drm_dev = match state.drm_devs.get(&dev_id) {
            Some(dev) => Some(dev),
            _ => panic!("connector's drm device does not exist"),
        };
    }
    let id = connector.id();
    let data = Rc::new(ConnectorData {
        connector: connector.clone(),
        handler: Default::default(),
        connected: Cell::new(false),
        name: connector.kernel_id().to_string(),
        drm_dev: drm_dev.clone(),
        async_event: Rc::new(AsyncEvent::default()),
    });
    if let Some(dev) = drm_dev {
        dev.connectors.set(id, data.clone());
    }
    let oh = ConnectorHandler {
        id,
        state: state.clone(),
        data: data.clone(),
    };
    let future = state.eng.spawn(oh.handle());
    data.handler.set(Some(future));
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
        if let Some(config) = self.state.config.get() {
            config.del_connector(self.id);
        }
        self.data.handler.set(None);
        self.state.connectors.remove(&self.id);
    }

    async fn handle_connected(&self, info: MonitorInfo) {
        log::info!("Connector {} connected", self.data.connector.kernel_id());
        self.data.connected.set(true);
        let name = self.state.globals.name();
        let x1 = self
            .state
            .root
            .outputs
            .lock()
            .values()
            .map(|o| o.global.pos.get().x2())
            .max()
            .unwrap_or(0);
        let global = Rc::new(WlOutputGlobal::new(
            name,
            &self.state,
            &self.data,
            x1,
            &info.initial_mode,
            &info.manufacturer,
            &info.product,
            &info.serial_number,
            info.width_mm,
            info.height_mm,
        ));
        let on = Rc::new(OutputNode {
            id: self.state.node_ids.next(),
            workspaces: Default::default(),
            workspace: CloneCell::new(None),
            seat_state: Default::default(),
            global: global.clone(),
            layers: Default::default(),
            render_data: RefCell::new(OutputRenderData {
                active_workspace: None,
                underline: Default::default(),
                inactive_workspaces: Default::default(),
                titles: Default::default(),
                status: None,
            }),
            state: self.state.clone(),
            is_dummy: false,
            status: self.state.status.clone(),
            scroll: Default::default(),
            pointer_positions: Default::default(),
            lock_surface: Default::default(),
        });
        let mode = info.initial_mode;
        let output_data = Rc::new(OutputData {
            connector: self.data.clone(),
            monitor_info: info,
            node: on.clone(),
        });
        self.state.outputs.set(self.id, output_data);
        if self.state.outputs.len() == 1 {
            let seats = self.state.globals.seats.lock();
            for seat in seats.values() {
                seat.set_position(x1 + mode.width / 2, mode.height / 2);
            }
        }
        global.node.set(Some(on.clone()));
        if let Some(config) = self.state.config.get() {
            config.connector_connected(self.id);
        }
        {
            for source in self.state.outputs.lock().values() {
                if source.node.id == on.id {
                    continue;
                }
                let mut ws_to_move = vec![];
                for ws in source.node.workspaces.iter() {
                    if ws.is_dummy {
                        continue;
                    }
                    if ws.desired_output.get() == global.output_id {
                        ws_to_move.push(ws.clone());
                    }
                }
                for ws in ws_to_move {
                    on.workspaces.add_last_existing(&ws);
                    if ws.visible_on_desired_output.get() && on.workspace.get().is_none() {
                        on.show_workspace(&ws);
                    } else {
                        ws.set_visible(false);
                    }
                    if let Some(visible) = source.node.workspace.get() {
                        if visible.id == ws.id {
                            source.node.workspace.take();
                        }
                    }
                }
                if source.node.workspace.get().is_none() {
                    if let Some(ws) = source.node.workspaces.first() {
                        source.node.show_workspace(&ws);
                    }
                }
                source.node.update_render_data();
            }
            if on.workspace.get().is_none() {
                if let Some(ws) = on.workspaces.first() {
                    on.show_workspace(&ws);
                }
            }
        }
        on.update_render_data();
        self.state.root.outputs.set(self.id, on.clone());
        self.state.root.update_extents();
        self.state.add_global(&global);
        'outer: loop {
            while let Some(event) = self.data.connector.event() {
                match event {
                    ConnectorEvent::Disconnected => break 'outer,
                    ConnectorEvent::ModeChanged(mode) => {
                        on.update_mode(mode);
                    }
                    ev => unreachable!("received unexpected event {:?}", ev),
                }
            }
            self.data.async_event.triggered().await;
        }
        log::info!("Connector {} disconnected", self.data.connector.kernel_id());
        if let Some(config) = self.state.config.get() {
            config.connector_disconnected(self.id);
        }
        global.node.set(None);
        global.destroyed.set(true);
        let _ = self.state.remove_global(&*global);
        self.state.root.outputs.remove(&self.id);
        self.data.connected.set(false);
        self.state.outputs.remove(&self.id);
        on.lock_surface.take();
        let mut target_is_dummy = false;
        let target = match self.state.outputs.lock().values().next() {
            Some(o) => o.node.clone(),
            _ => {
                target_is_dummy = true;
                self.state.dummy_output.get().unwrap()
            }
        };
        if !on.workspaces.is_empty() {
            for ws in on.workspaces.iter() {
                let is_visible =
                    !target_is_dummy && target.workspaces.is_empty() && ws.visible.get();
                ws.visible_on_desired_output.set(ws.visible.get());
                ws.output.set(target.clone());
                target.workspaces.add_last_existing(&ws);
                if is_visible {
                    target.show_workspace(&ws);
                } else if ws.visible.get() {
                    ws.set_visible(false);
                }
            }
            target.update_render_data();
            self.state.tree_changed();
            self.state.damage();
        }
        let seats = self.state.globals.seats.lock();
        for seat in seats.values() {
            if seat.get_output().id == on.id {
                let tpos = target.global.pos.get();
                let tmode = target.global.mode.get();
                seat.set_position(tpos.x1() + tmode.width / 2, tpos.y1() + tmode.height / 2);
            }
        }
        if let Some(dev) = &self.data.drm_dev {
            dev.connectors.remove(&self.id);
        }
    }
}
