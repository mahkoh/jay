use {
    super::{
        zwlr_output_configuration_v1::ZwlrOutputConfigurationV1,
        zwlr_output_head_v1::ZwlrOutputHeadV1,
    },
    crate::{
        client::{CAP_OUTPUT_MANAGER, Client, ClientCaps, ClientError},
        fixed::Fixed,
        globals::{Global, GlobalName},
        ifs::output_manager::zwlr_output_head_v1::{
            ADAPTIVE_SYNC_SINCE, MAKE_SINCE, MODEL_SINCE, SERIAL_NUMBER_SINCE,
        },
        leaks::Tracker,
        object::{Object, Version},
        tree::{Node, OutputNode},
        utils::opt::Opt,
        wire::{ZwlrOutputManagerV1Id, zwlr_output_manager_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

linear_ids!(OutputManagerIds, OutputManagerId, u64);

pub struct ZwlrOutputManagerV1Global {
    name: GlobalName,
}

pub struct ZwlrOutputManagerV1 {
    pub id: ZwlrOutputManagerV1Id,
    pub manager_id: OutputManagerId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub serial: Rc<Cell<u32>>,
    pub opt: Rc<Opt<ZwlrOutputManagerV1>>,
    pub done_scheduled: Cell<bool>,
    pub version: Version,
}

impl ZwlrOutputManagerV1 {
    fn detach(&self) {
        self.client
            .state
            .output_managers
            .managers
            .remove(&self.manager_id);
    }
}

impl ZwlrOutputManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwlrOutputManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwlrOutputManagerV1Error> {
        let obj = Rc::new(ZwlrOutputManagerV1 {
            id,
            manager_id: client.state.output_managers.ids.next(),
            client: client.clone(),
            tracker: Default::default(),
            serial: Rc::new(Cell::new(0)),
            version,
            opt: Default::default(),
            done_scheduled: Cell::new(false),
        });

        track!(client, obj);
        client.add_client_obj(&obj)?;
        obj.opt.set(Some(obj.clone()));
        client
            .state
            .output_managers
            .managers
            .set(obj.manager_id, obj.clone());
        for output in client.state.root.outputs.lock().values() {
            obj.announce_head(output);
        }
        Ok(())
    }
}

impl ZwlrOutputManagerV1RequestHandler for ZwlrOutputManagerV1 {
    type Error = ZwlrOutputManagerV1Error;

    fn create_configuration(
        &self,
        req: CreateConfiguration,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let configuration = Rc::new(ZwlrOutputConfigurationV1 {
            id: req.id,
            configuration_id: self.client.state.output_managers.configiuration_ids.next(),
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            serial: self.serial.get(),
            manager: self.opt.clone(),
            manager_id: self.manager_id,
            used: Cell::new(false),
        });

        track!(self.client, configuration);
        self.client.add_client_obj(&configuration)?;
        self.client
            .state
            .output_managers
            .configurations
            .set(configuration.configuration_id, configuration);
        Ok(())
    }

    fn stop(&self, _req: Stop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.send_finished();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

impl ZwlrOutputManagerV1 {
    pub fn announce_head(&self, node: &OutputNode) {
        let global = node.global.clone();
        let connector_data = global.connector.clone();
        let connector = connector_data.connector.clone();
        let width_mm = global.width_mm;
        let height_mm = global.height_mm;
        let output_id = global.output_id.clone();
        let enabled = connector.enabled();
        let name = &connector_data.name;
        let make = &output_id.manufacturer;
        let serial_number = &output_id.serial_number;
        let model = &output_id.model;
        let description = &format!("{} {} {}", make, name, model)
            .trim()
            .replace("  ", " ");
        let scale = Fixed::from_f64(global.persistent.scale.get().to_f64());
        let modes = global.modes.clone();
        let current_mode = global.mode.get();
        let transform = global.persistent.transform.get();
        let adaptive_sync_enabled = node.schedule.vrr_enabled();

        let (x, y) = node.node_absolute_position().position();

        let id = match self.client.new_id() {
            Ok(id) => id,
            Err(e) => {
                self.client.error(e);
                return;
            }
        };
        let head = Rc::new(ZwlrOutputHeadV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            manager_id: self.manager_id,
            manager: self.opt.clone(),
            output: global.opt.clone(),
            opt: Default::default(),
            configuration_heads: Default::default(),
            modes: Default::default(),
            current_mode: Default::default(),
        });
        track!(self.client, head);
        self.client.add_server_obj(&head);
        head.opt.set(Some(head.clone()));
        node.zwlr_output_heads.set(self.manager_id, head.clone());
        self.send_head(&head);

        head.send_enabled(enabled);

        head.publish_mode(current_mode, true);

        let mut preferred = current_mode;
        for mode in modes {
            let _ = match head.publish_mode(mode, false) {
                Some(mode) => mode,
                None => continue,
            };
            if mode.height > preferred.height && mode.width > preferred.width {
                preferred = mode;
            } else if (preferred.width, preferred.height) == (mode.width, mode.height) {
                if mode.refresh_rate_millihz > preferred.refresh_rate_millihz {
                    preferred = mode;
                }
            }
        }
        for mode in head.modes.lock().values() {
            let refresh = mode.refresh.get().unwrap_or_default();
            let width = mode.width.get();
            let height = mode.height.get();

            if (width, height, refresh as u32)
                == (
                    preferred.width,
                    preferred.height,
                    preferred.refresh_rate_millihz,
                )
            {
                mode.send_preferred();
            }
        }
        head.send_physical_size(width_mm, height_mm);
        head.send_scale(scale);
        head.send_position(x, y);
        head.send_transform(transform);
        if !name.is_empty() {
            head.send_name(name);
        }
        if !description.is_empty() {
            head.send_description(description);
        }
        if !make.is_empty() && self.version >= MAKE_SINCE {
            head.send_make(make);
        }
        if !model.is_empty() && self.version >= MODEL_SINCE {
            head.send_model(model);
        }
        if !serial_number.is_empty() && self.version >= SERIAL_NUMBER_SINCE {
            head.send_serial_number(serial_number);
        }
        if self.version >= ADAPTIVE_SYNC_SINCE {
            head.send_adaptive_sync(adaptive_sync_enabled);
        }
        node.zwlr_output_heads.set(self.manager_id, head.clone());

        self.schedule_done();
    }

    pub fn send_head(&self, head: &ZwlrOutputHeadV1) {
        self.client.event(Head {
            self_id: self.id,
            head: head.id,
        });
    }

    pub fn send_done(&self, serial: u32) {
        self.serial.set(serial);
        self.done_scheduled.set(false);
        self.client.event(Done {
            self_id: self.id,
            serial,
        });
    }
    pub fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id });
    }

    pub(super) fn schedule_done(&self) {
        if self.done_scheduled.replace(true) {
            return;
        }
        self.client
            .state
            .output_managers
            .queue
            .push(self.opt.clone());
    }
}

global_base!(
    ZwlrOutputManagerV1Global,
    ZwlrOutputManagerV1,
    ZwlrOutputManagerV1Error
);

impl Global for ZwlrOutputManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        4
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_OUTPUT_MANAGER
    }
}

simple_add_global!(ZwlrOutputManagerV1Global);

object_base! {
    self = ZwlrOutputManagerV1;
    version = self.version;
}

simple_add_obj!(ZwlrOutputManagerV1);

impl Object for ZwlrOutputManagerV1 {}

#[derive(Debug, Error)]
pub enum ZwlrOutputManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrOutputManagerV1Error, ClientError);
