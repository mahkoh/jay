use {
    super::zwlr_output_configuration_head::ZwlrOutputConfigurationHeadV1,
    crate::{
        client::{Client, ClientError},
        ifs::output_manager::{
            zwlr_output_configuration_head::OutputConfig,
            zwlr_output_head_v1::ZwlrOutputHeadV1,
            zwlr_output_manager_v1::{OutputManagerId, ZwlrOutputManagerV1},
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        scale::Scale,
        tree::{Node, VrrMode},
        utils::opt::Opt,
        wire::{ZwlrOutputConfigurationV1Id, zwlr_output_configuration_v1::*},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
};

linear_ids!(OutputConfigurationIds, OutputConfigurationId, u64);

pub struct ZwlrOutputConfigurationV1 {
    pub id: ZwlrOutputConfigurationV1Id,
    pub configuration_id: OutputConfigurationId,
    pub version: Version,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub serial: u32,
    pub manager_id: OutputManagerId,
    pub manager: Rc<Opt<ZwlrOutputManagerV1>>,
    pub used: Cell<bool>,
}

impl ZwlrOutputConfigurationV1 {
    fn detach(&self) {
        for output in self.client.state.root.outputs.lock().values() {
            let head = output.zwlr_output_heads.get(&self.manager_id);
            if let Some(head) = head {
                head.configuration_heads.remove(&self.configuration_id);
            }
        }
        self.client
            .state
            .output_managers
            .configurations
            .remove(&self.configuration_id);
    }
    pub fn send_succeeded(&self) {
        self.client.event(Succeeded { self_id: self.id });
    }

    pub fn send_failed(&self) {
        self.client.event(Failed { self_id: self.id });
    }

    pub fn send_cancelled(&self) {
        self.client.event(Cancelled { self_id: self.id });
    }

    fn reverse_changes(&self, old: Vec<(Rc<ZwlrOutputHeadV1>, Rc<RefCell<OutputConfig>>)>) {
        let mut changed = false;
        for (head, config) in old {
            let node = match head.output.node() {
                Some(node) => node,
                None => return,
            };
            let connector = match head.output.get() {
                Some(o) => o.connector.connector.clone(),
                None => return,
            };

            let enabled = match head.configuration_heads.get(&self.configuration_id) {
                Some((enabled, _)) => enabled,
                None => continue,
            };

            connector.set_enabled(*enabled);

            if !*enabled {
                continue;
            }

            let config = config.borrow();
            if config.x.is_some() || config.y.is_some() {
                let (old_x, old_y) = node.global.position().position();
                let x = config.x.unwrap_or(old_x);
                let y = config.y.unwrap_or(old_y);
                node.set_position(x, y);
                node.global.update_damage_matrix();
                changed = true;
            }
            if let Some(scale) = config.scale {
                node.set_preferred_scale(Scale::from_f64(scale));
            }
            if let Some(transform) = config.transform {
                node.update_transform(transform);
                node.global.update_damage_matrix();
                changed = true;
            }
            if let Some(mode) = config.mode {
                connector.set_mode(mode);
            }
            if let Some(enabled) = config.vrr_enabled {
                let vrr_mode = if enabled {
                    VrrMode::VARIANT_1
                } else {
                    VrrMode::NEVER
                };
                node.global.persistent.vrr_mode.set(&vrr_mode);
                node.update_presentation_type();
                changed = true;
            }
        }
        if let Some(manager) = self.manager.get() {
            if changed {
                manager.schedule_done();
            }
        }
    }
}

impl ZwlrOutputConfigurationV1RequestHandler for ZwlrOutputConfigurationV1 {
    type Error = ZwlrOutputConfigurationV1Error;

    fn enable_head(&self, req: EnableHead, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.used.get() {
            return Err(ZwlrOutputConfigurationV1Error::AlreadyUsed);
        }
        let head = self.client.lookup(req.head);
        if let Ok(head) = head {
            if head.configuration_heads.contains(&self.configuration_id) {
                return Err(Self::Error::AlreadyConfiguredHead);
            }

            let configuration_head = Rc::new(ZwlrOutputConfigurationHeadV1 {
                id: req.id,
                version: self.version,
                client: self.client.clone(),
                head: head.opt.clone(),
                config: Default::default(),
                tracker: Default::default(),
            });

            track!(self.client, configuration_head);
            self.client.add_client_obj(&configuration_head)?;

            head.configuration_heads.set(
                self.configuration_id,
                (Rc::new(true), Some(configuration_head)),
            );
        }
        Ok(())
    }

    fn disable_head(&self, req: DisableHead, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.used.get() {
            return Err(ZwlrOutputConfigurationV1Error::AlreadyUsed);
        }
        let head = self.client.lookup(req.head);
        if let Ok(head) = head {
            if head.configuration_heads.contains(&self.configuration_id) {
                return Err(Self::Error::AlreadyConfiguredHead);
            }

            head.configuration_heads
                .set(self.configuration_id, (Rc::new(false), None));
        }
        Ok(())
    }

    fn apply(&self, _req: Apply, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.used.get() {
            return Err(ZwlrOutputConfigurationV1Error::AlreadyUsed);
        }

        let manager = match self.manager.get() {
            Some(manager) => manager,
            None => {
                self.send_failed();
                return Ok(());
            }
        };

        if self.serial < manager.serial.get() {
            self.send_cancelled();
            return Ok(());
        }

        self.used.set(true);

        if self.client.state.root.outputs.lock().values().any(|o| {
            !o.zwlr_output_heads
                .get(&manager.manager_id)
                .map(|h| h.configuration_heads.contains(&self.configuration_id))
                .unwrap_or(false)
        }) {
            return Err(ZwlrOutputConfigurationV1Error::UnconfiguredHead);
        }

        let mut changed = false;
        let mut old_configs = vec![];
        let outputs = self.client.state.root.outputs.lock();
        for head in outputs
            .values()
            .filter_map(|on| on.zwlr_output_heads.get(&self.manager_id))
            .clone()
        {
            let node = match head.output.node() {
                Some(node) => node,
                None => {
                    self.reverse_changes(old_configs);
                    self.send_failed();
                    return Ok(());
                }
            };
            let connector = match head.output.get() {
                Some(o) => o.connector.connector.clone(),
                None => {
                    self.reverse_changes(old_configs);
                    self.send_failed();
                    return Ok(());
                }
            };
            let change: Rc<RefCell<OutputConfig>> = Rc::new(RefCell::new(Default::default()));
            old_configs.push((head.clone(), change.clone()));

            let (enabled, configuration) =
                match head.configuration_heads.get(&self.configuration_id) {
                    Some((enabled, configuration)) => match configuration {
                        Some(c) => (enabled, c),
                        None => continue,
                    },
                    None => continue,
                };

            connector.set_enabled(*enabled);

            if !*enabled {
                continue;
            }

            let config = configuration.config.borrow();

            if let Some(scale) = config.scale {
                change.borrow_mut().scale = Some(node.global.persistent.scale.get().to_f64());
                node.set_preferred_scale(Scale::from_f64(scale));
                changed = true;
            }
            if let Some(transform) = config.transform {
                change.borrow_mut().transform = Some(node.global.persistent.transform.get());
                node.update_transform(transform);
                node.global.update_damage_matrix();
                changed = true;
            }
            if let Some(mode) = config.mode {
                change.borrow_mut().mode = Some(node.global.mode.get());
                let modes = &node.global.modes;
                let current_mode = node.global.mode.get();
                let m = modes.iter().find(|m| {
                    if m.width != mode.width
                        || m.height != mode.height
                        || m.refresh_rate_millihz != mode.refresh_rate_millihz
                    {
                        return false;
                    } else {
                        return true;
                    }
                });
                match m.cloned() {
                    None => {
                        if current_mode.width != mode.width
                            || current_mode.height != mode.height
                            || current_mode.refresh_rate_millihz != mode.refresh_rate_millihz
                        {
                            self.send_failed();
                            self.reverse_changes(old_configs);
                            return Ok(());
                        } else {
                            connector.set_mode(current_mode);
                            changed = true;
                        }
                    }
                    Some(m) => {
                        connector.set_mode(m);
                        changed = true;
                    }
                }
            }
            if config.x.is_some() || config.y.is_some() {
                let (old_x, old_y) = node.global.position().position();
                change.borrow_mut().x = Some(old_x);
                change.borrow_mut().y = Some(old_y);
                let x = config.x.unwrap_or(old_x);
                let y = config.y.unwrap_or(old_y);
                node.set_position(x, y);
                node.global.update_damage_matrix();
                changed = true;
            }
            if let Some(enabled) = config.vrr_enabled {
                change.borrow_mut().vrr_enabled = Some(node.schedule.vrr_enabled());
                let vrr_mode = if enabled {
                    let default_mode = self.client.state.default_vrr_mode.get();
                    if default_mode == VrrMode::NEVER {
                        VrrMode::VARIANT_1
                    } else {
                        default_mode
                    }
                } else {
                    VrrMode::NEVER
                };
                node.global.persistent.vrr_mode.set(&vrr_mode);
                node.update_presentation_type();
                changed = true;
            }
        }
        for output in outputs.values() {
            let (x, y) = output.global.position().position();
            let scale = output.global.persistent.scale.get().to_f64();
            let (width, height) = output.global.pixel_size();
            let new_rect = Rect::new_sized(
                x,
                y,
                (width as f64 / scale).round() as i32,
                (height as f64 / scale).round() as i32,
            )
            .unwrap();
            for other in outputs.values() {
                if other.node_id() == output.node_id() {
                    continue;
                }
                let scale = other.global.persistent.scale.get().to_f64();
                let (x, y) = other.global.position().position();
                let (width, height) = other.global.pixel_size();
                let rect = Rect::new_sized(
                    x,
                    y,
                    (width as f64 / scale).round() as i32,
                    (height as f64 / scale).round() as i32,
                )
                .unwrap();
                if rect.intersects(&new_rect) {
                    self.send_failed();
                    self.reverse_changes(old_configs);
                    return Ok(());
                }
            }
        }

        self.send_succeeded();
        if changed {
            let serial = manager.serial.get() + 1;
            manager.send_done(serial);
        }

        Ok(())
    }

    fn test(&self, _req: Test, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.used.get() {
            return Err(ZwlrOutputConfigurationV1Error::AlreadyUsed);
        }

        let manager = match self.manager.get() {
            Some(manager) => manager,
            None => {
                self.send_cancelled();
                return Ok(());
            }
        };

        if self.client.state.root.outputs.lock().values().any(|o| {
            !o.zwlr_output_heads
                .get(&manager.manager_id)
                .map(|h| h.configuration_heads.contains(&self.configuration_id))
                .unwrap_or(false)
        }) {
            return Err(ZwlrOutputConfigurationV1Error::UnconfiguredHead);
        }

        self.used.set(true);

        for head in self
            .client
            .state
            .root
            .outputs
            .lock()
            .values()
            .filter_map(|on| on.zwlr_output_heads.get(&self.manager_id))
        {
            let node = match head.output.node() {
                Some(node) => node,
                None => {
                    self.send_failed();
                    return Ok(());
                }
            };
            match head.output.get() {
                Some(o) => o.connector.connector.clone(),
                None => {
                    self.send_failed();
                    return Ok(());
                }
            };

            let (enabled, configuration) =
                match head.configuration_heads.get(&self.configuration_id) {
                    Some((enabled, configuration)) => match configuration {
                        Some(c) => (enabled, c),
                        None => continue,
                    },
                    None => continue,
                };

            if !*enabled {
                continue;
            }

            let config = configuration.config.borrow();

            if let Some(mode) = config.mode {
                let modes = &node.global.modes;
                let m = modes.iter().find(|m| {
                    if m.width != mode.width
                        || m.height != mode.height
                        || m.refresh_rate_millihz != mode.refresh_rate_millihz
                    {
                        return false;
                    } else {
                        return true;
                    }
                });
                match m.cloned() {
                    None => {
                        self.send_failed();
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
        self.send_succeeded();
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrOutputConfigurationV1;
    version = self.version;
}

impl Object for ZwlrOutputConfigurationV1 {}

simple_add_obj!(ZwlrOutputConfigurationV1);

#[derive(Debug, Error)]
pub enum ZwlrOutputConfigurationV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("head has been configured twice")]
    AlreadyConfiguredHead,
    #[error("head has not been configured")]
    UnconfiguredHead,
    #[error("request sent after configuration has been applied or tested")]
    AlreadyUsed,
}
efrom!(ZwlrOutputConfigurationV1Error, ClientError);
