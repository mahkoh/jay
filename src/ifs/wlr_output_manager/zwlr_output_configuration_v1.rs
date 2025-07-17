use {
    crate::{
        backend::{
            ConnectorId,
            transaction::{
                BackendConnectorTransactionError, ConnectorTransaction,
                PreparedConnectorTransaction,
            },
        },
        client::{Client, ClientError},
        ifs::wlr_output_manager::{
            zwlr_output_configuration_head::ZwlrOutputConfigurationHeadV1,
            zwlr_output_manager_v1::ZwlrOutputManagerV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::{copyhashmap::CopyHashMap, errorfmt::ErrorFmt, hash_map_ext::HashMapExt},
        wire::{ZwlrOutputConfigurationV1Id, zwlr_output_configuration_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwlrOutputConfigurationV1 {
    pub(super) id: ZwlrOutputConfigurationV1Id,
    pub(super) version: Version,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) serial: u64,
    pub(super) manager: Rc<ZwlrOutputManagerV1>,
    pub(super) used: Cell<bool>,
    pub(super) enabled_outputs: CopyHashMap<ConnectorId, Rc<ZwlrOutputConfigurationHeadV1>>,
    pub(super) configured_outputs: CopyHashMap<ConnectorId, ()>,
}

#[derive(Debug, Error)]
enum ConfigError {
    #[error("Serial is out of date")]
    OutOfDate,
    #[error("Unconfigured output {0}")]
    UnconfiguredOutput(Rc<String>),
    #[error("Could not add output to transaction")]
    AddToTransaction(#[source] BackendConnectorTransactionError),
    #[error("Could not prepare transaction")]
    PrepareTransaction(#[source] BackendConnectorTransactionError),
    #[error("Could not apply transaction")]
    ApplyTransaction(#[source] BackendConnectorTransactionError),
}

impl ZwlrOutputConfigurationV1 {
    pub fn send_succeeded(&self) {
        self.client.event(Succeeded { self_id: self.id });
    }

    pub fn send_failed(&self) {
        self.client.event(Failed { self_id: self.id });
    }

    pub fn send_cancelled(&self) {
        self.client.event(Cancelled { self_id: self.id });
    }

    fn assert_unused(&self) -> Result<(), ZwlrOutputConfigurationV1Error> {
        if self.used.get() {
            return Err(ZwlrOutputConfigurationV1Error::AlreadyUsed);
        }
        Ok(())
    }

    fn prepare_transaction(&self) -> Result<PreparedConnectorTransaction, ConfigError> {
        if self.serial < self.manager.serial.get() {
            return Err(ConfigError::OutOfDate);
        }
        let mut tran = ConnectorTransaction::new(&self.client.state);
        for output in self.client.state.outputs.lock().values() {
            let mut state = output.connector.state.get();
            match self.enabled_outputs.get(&output.connector.id) {
                None => {
                    if self.configured_outputs.not_contains(&output.connector.id) {
                        return Err(ConfigError::UnconfiguredOutput(
                            output.connector.name.clone(),
                        ));
                    }
                    state.enabled = false;
                }
                Some(config) => {
                    state.enabled = true;
                    let config = *config.config.borrow();
                    if let Some(mode) = config.mode {
                        state.mode = mode;
                    }
                }
            }
            tran.add(&output.connector.connector, state)
                .map_err(ConfigError::AddToTransaction)?;
        }
        tran.prepare().map_err(ConfigError::PrepareTransaction)
    }

    fn apply_transaction(&self) -> Result<(), ConfigError> {
        self.prepare_transaction()?
            .apply()
            .map_err(ConfigError::ApplyTransaction)?
            .commit();
        for output in self.client.state.outputs.lock().values() {
            let Some(config) = self.enabled_outputs.get(&output.connector.id) else {
                continue;
            };
            let config = *config.config.borrow();
            if let Some(node) = &output.node {
                if let Some(v) = config.transform {
                    node.update_transform(v);
                }
                if let Some(v) = config.scale {
                    node.set_preferred_scale(v);
                }
                if let Some(v) = config.vrr_mode {
                    node.set_vrr_mode(v);
                }
                if let Some(v) = config.pos {
                    node.set_position(v.0, v.1);
                }
            } else {
                let mi = &output.monitor_info;
                let pos = &self.client.state.persistent_output_states;
                let pos = pos.lock().entry(mi.output_id.clone()).or_default().clone();
                if let Some(v) = config.transform {
                    pos.transform.set(v);
                }
                if let Some(v) = config.scale {
                    pos.scale.set(v);
                }
                if let Some(v) = config.vrr_mode {
                    pos.vrr_mode.set(v);
                }
                if let Some(v) = config.pos {
                    pos.pos.set(v);
                }
            }
        }
        Ok(())
    }
}

impl ZwlrOutputConfigurationV1RequestHandler for ZwlrOutputConfigurationV1 {
    type Error = ZwlrOutputConfigurationV1Error;

    fn enable_head(&self, req: EnableHead, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.assert_unused()?;
        let head = self.client.lookup(req.head)?;
        if self.configured_outputs.set(head.connector_id, ()).is_some() {
            return Err(ZwlrOutputConfigurationV1Error::AlreadyConfiguredHead(
                head.output.connector.name.clone(),
            ));
        }
        let configuration_head = Rc::new(ZwlrOutputConfigurationHeadV1 {
            id: req.id,
            head_id: head.head_id,
            version: self.version,
            client: self.client.clone(),
            config: Default::default(),
            tracker: Default::default(),
        });
        track!(self.client, configuration_head);
        self.client.add_client_obj(&configuration_head)?;
        self.enabled_outputs
            .set(head.connector_id, configuration_head);
        Ok(())
    }

    fn disable_head(&self, req: DisableHead, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.assert_unused()?;
        let head = self.client.lookup(req.head)?;
        if self.configured_outputs.set(head.connector_id, ()).is_some() {
            return Err(ZwlrOutputConfigurationV1Error::AlreadyConfiguredHead(
                head.output.connector.name.clone(),
            ));
        }
        Ok(())
    }

    fn apply(&self, _req: Apply, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.assert_unused()?;
        self.used.set(true);
        let Err(e) = self.apply_transaction() else {
            self.send_succeeded();
            return Ok(());
        };
        log::error!("Could not apply output configuration: {}", ErrorFmt(&e));
        match e {
            ConfigError::UnconfiguredOutput(o) => {
                return Err(ZwlrOutputConfigurationV1Error::UnconfiguredHead(o));
            }
            ConfigError::OutOfDate => {
                self.send_cancelled();
                return Ok(());
            }
            ConfigError::AddToTransaction(_)
            | ConfigError::PrepareTransaction(_)
            | ConfigError::ApplyTransaction(_) => {}
        }
        self.send_failed();
        Ok(())
    }

    fn test(&self, _req: Test, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.assert_unused()?;
        self.used.set(true);
        let Err(e) = self.prepare_transaction() else {
            self.send_succeeded();
            return Ok(());
        };
        log::error!("Could not test output configuration: {}", ErrorFmt(&e));
        match e {
            ConfigError::UnconfiguredOutput(o) => {
                return Err(ZwlrOutputConfigurationV1Error::UnconfiguredHead(o));
            }
            ConfigError::OutOfDate
            | ConfigError::AddToTransaction(_)
            | ConfigError::PrepareTransaction(_)
            | ConfigError::ApplyTransaction(_) => {}
        }
        self.send_failed();
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        for head in self.enabled_outputs.lock().drain_values() {
            self.client.remove_obj(&*head)?;
        }
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
    #[error("Head {0} has alread been configured")]
    AlreadyConfiguredHead(Rc<String>),
    #[error("Head {0} has not been configured")]
    UnconfiguredHead(Rc<String>),
    #[error("Configuration has already been tested or applied")]
    AlreadyUsed,
}
efrom!(ZwlrOutputConfigurationV1Error, ClientError);
