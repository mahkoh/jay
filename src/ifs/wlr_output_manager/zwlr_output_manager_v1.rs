use {
    crate::{
        client::{CAP_HEAD_MANAGER, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::wlr_output_manager::{
            zwlr_output_configuration_v1::ZwlrOutputConfigurationV1,
            zwlr_output_head_v1::{
                ADAPTIVE_SYNC_SINCE, MAKE_SINCE, MODEL_SINCE, SERIAL_NUMBER_SINCE, ZwlrOutputHeadV1,
            },
            zwlr_output_mode_v1::ZwlrOutputModeV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        state::OutputData,
        utils::numcell::NumCell,
        wire::{ZwlrOutputManagerV1Id, zwlr_output_manager_v1::*},
    },
    ahash::AHashMap,
    isnt::std_1::string::IsntStringExt,
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

linear_ids!(WlrOutputManagerIds, WlrOutputManagerId, u64);

pub struct ZwlrOutputManagerV1Global {
    name: GlobalName,
}

pub struct ZwlrOutputManagerV1 {
    pub(super) id: ZwlrOutputManagerV1Id,
    pub(super) manager_id: WlrOutputManagerId,
    pub(super) client: Rc<Client>,
    pub(super) version: Version,
    pub(super) tracker: Tracker<Self>,
    pub(super) done_scheduled: Cell<bool>,
    pub(super) serial: NumCell<u64>,
    pub(super) destroyed: Cell<bool>,
}

impl ZwlrOutputManagerV1 {
    fn detach(&self) {
        self.client
            .state
            .wlr_output_managers
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
            manager_id: client.state.wlr_output_managers.ids.next(),
            client: client.clone(),
            tracker: Default::default(),
            version,
            done_scheduled: Cell::new(false),
            serial: Default::default(),
            destroyed: Cell::new(false),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        client
            .state
            .wlr_output_managers
            .managers
            .set(obj.manager_id, obj.clone());
        for output in client.state.outputs.lock().values() {
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
        slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let last_serial = self.serial.get();
        let mut serial = (last_serial >> u32::BITS << u32::BITS) | (req.serial as u64);
        if serial > last_serial {
            serial = serial.saturating_sub(1 << u32::BITS);
        }
        let configuration = Rc::new(ZwlrOutputConfigurationV1 {
            id: req.id,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            serial,
            manager: slf.clone(),
            used: Cell::new(false),
            enabled_outputs: Default::default(),
            configured_outputs: Default::default(),
        });
        track!(self.client, configuration);
        self.client.add_client_obj(&configuration)?;
        Ok(())
    }

    fn stop(&self, _req: Stop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroyed.set(true);
        self.detach();
        self.send_finished();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

impl ZwlrOutputManagerV1 {
    pub fn announce_head(self: &Rc<Self>, output: &Rc<OutputData>) {
        let id = match self.client.new_id() {
            Ok(id) => id,
            Err(e) => {
                self.client.error(e);
                return;
            }
        };
        let mi = &output.monitor_info;
        let state = output.connector.state.get();
        let head_id = self.client.state.wlr_output_managers.head_ids.next();
        let mut modes_list = vec![];
        let mut modes = AHashMap::new();
        let mut have_current = false;
        for (idx, mode) in mi.modes.iter().enumerate() {
            if modes.contains_key(mode) {
                continue;
            }
            let current = !have_current && *mode == state.mode;
            if current {
                have_current = true;
            }
            let id = match self.client.new_id() {
                Ok(id) => id,
                Err(e) => {
                    self.client.error(e);
                    return;
                }
            };
            let output_mode = Rc::new(ZwlrOutputModeV1 {
                id,
                head_id,
                client: self.client.clone(),
                tracker: Default::default(),
                version: self.version,
                mode: *mode,
                preferred: idx == 0,
                initial_current: current,
                destroyed: Cell::new(false),
            });
            track!(self.client, output_mode);
            self.client.add_server_obj(&output_mode);
            modes_list.push(output_mode.clone());
            modes.insert(*mode, output_mode);
        }
        let head = Rc::new(ZwlrOutputHeadV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            manager_id: self.manager_id,
            manager: self.clone(),
            head_id,
            connector_id: output.connector.id,
            modes,
            output: output.clone(),
        });
        track!(self.client, head);
        self.client.add_server_obj(&head);
        output
            .connector
            .wlr_output_heads
            .set(self.manager_id, head.clone());
        self.send_head(&head);
        head.send_name(&output.connector.name);
        let description = &*output.connector.description.borrow();
        if description.is_not_empty() {
            head.send_description(description);
        }
        head.send_enabled(!mi.non_desktop_effective);
        head.announce_modes(&modes_list);
        head.send_physical_size(mi.width_mm, mi.height_mm);
        if mi.output_id.manufacturer.is_not_empty() && head.version >= MAKE_SINCE {
            head.send_make(&mi.output_id.manufacturer);
        }
        if mi.output_id.model.is_not_empty() && head.version >= MODEL_SINCE {
            head.send_model(&mi.output_id.model);
        }
        if mi.output_id.serial_number.is_not_empty() && head.version >= SERIAL_NUMBER_SINCE {
            head.send_serial_number(&mi.output_id.serial_number);
        }
        if let Some(node) = &output.node {
            let p = &node.global.persistent;
            head.send_scale(p.scale.get());
            head.send_position(p.pos.get().0, p.pos.get().1);
            head.send_transform(p.transform.get());
            if head.version >= ADAPTIVE_SYNC_SINCE {
                head.send_adaptive_sync(p.vrr_mode.get());
            }
        }
        self.schedule_done();
    }

    pub fn send_head(&self, head: &ZwlrOutputHeadV1) {
        self.client.event(Head {
            self_id: self.id,
            head: head.id,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done {
            self_id: self.id,
            serial: self.serial.get() as u32,
        });
    }

    pub fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id });
    }

    pub(super) fn schedule_done(self: &Rc<Self>) {
        if self.done_scheduled.replace(true) {
            return;
        }
        self.serial.fetch_add(1);
        self.client
            .state
            .wlr_output_managers
            .queue
            .push(self.clone());
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
        CAP_HEAD_MANAGER
    }
}

simple_add_global!(ZwlrOutputManagerV1Global);

object_base! {
    self = ZwlrOutputManagerV1;
    version = self.version;
}

simple_add_obj!(ZwlrOutputManagerV1);

impl Object for ZwlrOutputManagerV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

#[derive(Debug, Error)]
pub enum ZwlrOutputManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrOutputManagerV1Error, ClientError);
