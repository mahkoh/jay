use {
    super::zwlr_output_configuration_head::ZwlrOutputConfigurationHeadV1,
    crate::{
        client::{Client, ClientError},
        ifs::output_manager::zwlr_output_manager_v1::{OutputManagerId, ZwlrOutputManagerV1},
        leaks::Tracker,
        object::{Object, Version},
        scale::Scale,
        utils::opt::Opt,
        wire::{ZwlrOutputConfigurationV1Id, zwlr_output_configuration_v1::*},
    },
    std::{cell::Cell, rc::Rc},
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
    pub applying: Cell<bool>,
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
                transform: Default::default(),
                scale: Default::default(),
                vrr_enabled: Default::default(),
                x: Default::default(),
                y: Default::default(),
                mode: Default::default(),
                custom_mode: Default::default(),
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
                self.send_cancelled();
                return Ok(());
            }
        };

        if self.serial < manager.serial.get() {
            self.send_cancelled();
            return Ok(());
        }

        self.used.set(true);
        self.applying.set(true);

        if self.client.state.root.outputs.lock().values().any(|o| {
            !o.zwlr_output_heads
                .get(&manager.manager_id)
                .map(|h| h.configuration_heads.contains(&self.configuration_id))
                .unwrap_or(false)
        }) {
            return Err(ZwlrOutputConfigurationV1Error::UnconfiguredHead);
        }

        let mut changed = false;
        for head in self.client.objects.zwlr_output_heads.lock().values() {
            let node = match head.output.node() {
                Some(node) => node,
                None => {
                    println!("sending failed for head none");
                    self.send_failed();
                    return Ok(());
                }
            };
            let connector = match head.output.get() {
                Some(o) => o.connector.connector.clone(),
                None => {
                    self.send_failed();
                    return Ok(());
                }
            };

            let (enabled, configuration) = head
                .configuration_heads
                .get(&self.configuration_id)
                .unwrap();

            connector.set_enabled(*enabled);

            if !*enabled {
                continue;
            }

            let configuration = configuration.unwrap();

            if configuration.x.get().is_some() || configuration.y.get().is_some() {
                let (old_x, old_y) = node.global.position().position();
                node.set_position(
                    configuration.x.get().unwrap_or(old_x),
                    configuration.y.get().unwrap_or(old_y),
                );
                changed = true;
            }
            if let Some(scale) = configuration.scale.get() {
                node.set_preferred_scale(Scale::from_f64(scale));
                changed = true;
            }
            if let Some(transform) = configuration.transform.get() {
                node.update_transform(transform);
                changed = true;
            }
            if let Some(mode) = configuration.mode.get() {
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
            if let Some(enabled) = configuration.vrr_enabled.get() {
                connector.set_vrr_enabled(enabled);
                changed = true;
            }
        }

        self.send_succeeded();
        if changed {
            manager.schedule_done();
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

        for head in self.client.objects.zwlr_output_heads.lock().values() {
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

            let (enabled, configuration) = head
                .configuration_heads
                .get(&self.configuration_id)
                .unwrap();

            if !*enabled {
                continue;
            }

            let configuration = configuration.unwrap();

            if let Some(mode) = configuration.mode.get() {
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
