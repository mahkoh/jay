use {
    super::HeadMgrCommon,
    crate::{
        client::{CAP_HEAD_MANAGER, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::head_management::{
            HeadCommon, HeadCommonError, HeadExtension, HeadMgrState, HeadName,
            jay_head_ext::{
                jay_head_ext_compositor_space_info_v1::JayHeadManagerExtCompositorSpaceInfoV1,
                jay_head_ext_compositor_space_positioner_v1::JayHeadManagerExtCompositorSpacePositionerV1,
                jay_head_ext_compositor_space_transformer_v1::JayHeadManagerExtCompositorSpaceTransformerV1,
                jay_head_ext_connector_info_v1::JayHeadManagerExtConnectorInfoV1,
                jay_head_ext_connector_settings_v1::JayHeadManagerExtConnectorSettingsV1,
                jay_head_ext_core_info_v1::JayHeadManagerExtCoreInfoV1,
                jay_head_ext_physical_display_info_v1::JayHeadManagerExtPhysicalDisplayInfoV1,
            },
            jay_head_transaction_v1::JayHeadTransactionV1,
            jay_head_v1::JayHeadV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        state::{ConnectorData, State},
        utils::{clonecell::CloneCell, numcell::NumCell},
        wire::{
            JayHeadManagerV1Id,
            jay_head_manager_v1::{
                BindExtension, CreateTransaction, Done, Extension, ExtensionsDone, HeadComplete,
                HeadStart, JayHeadManagerV1RequestHandler, Start, Stop, Stopped,
            },
        },
    },
    linearize::{Linearize, LinearizeExt},
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct JayHeadManagerV1Global {
    pub name: GlobalName,
}

impl JayHeadManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: JayHeadManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), JayHeadManagerV1Error> {
        let mgr = Rc::new(JayHeadManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            common: Rc::new(HeadMgrCommon {
                state: Default::default(),
            }),
            serial: Default::default(),
            done_scheduled: Cell::new(false),
            core_info_v1: Default::default(),
            compositor_space_info_v1: Default::default(),
            compositor_space_positioner_v1: Default::default(),
            compositor_space_transformer_v1: Default::default(),
            physical_display_info_v1: Default::default(),
            connector_info_v1: Default::default(),
            connector_settings_v1: Default::default(),
        });
        track!(client, mgr);
        client.add_client_obj(&mgr)?;
        macro_rules! ext {
            ($camel:ident) => {
                with_builtin_macros::with_eager_expansions! {
                    mgr.send_extension(
                        HeadExtension::$camel.linearize() as _,
                        #{concat_idents!(JayHeadManagerExt, $camel)}::NAME,
                        #{concat_idents!(JayHeadManagerExt, $camel)}::VERSION,
                    );
                }
            };
        }
        ext!(CoreInfoV1);
        ext!(CompositorSpaceInfoV1);
        ext!(CompositorSpacePositionerV1);
        ext!(CompositorSpaceTransformerV1);
        ext!(ConnectorInfoV1);
        ext!(ConnectorSettingsV1);
        ext!(PhysicalDisplayInfoV1);
        mgr.send_extensions_done();
        Ok(())
    }
}

global_base!(
    JayHeadManagerV1Global,
    JayHeadManagerV1,
    JayHeadManagerV1Error
);

simple_add_global!(JayHeadManagerV1Global);

impl Global for JayHeadManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_HEAD_MANAGER
    }
}

pub async fn handle_jay_head_manager_done(state: Rc<State>) {
    loop {
        let mgr = state.head_managers_done.pop().await;
        mgr.done_scheduled.set(false);
        if mgr.common.state.get() == HeadMgrState::Started {
            mgr.send_done();
        }
    }
}

pub struct JayHeadManagerV1 {
    pub id: JayHeadManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub common: Rc<HeadMgrCommon>,
    pub serial: NumCell<u64>,
    pub done_scheduled: Cell<bool>,
    pub core_info_v1: CloneCell<Option<Rc<JayHeadManagerExtCoreInfoV1>>>,
    pub compositor_space_info_v1: CloneCell<Option<Rc<JayHeadManagerExtCompositorSpaceInfoV1>>>,
    pub compositor_space_positioner_v1:
        CloneCell<Option<Rc<JayHeadManagerExtCompositorSpacePositionerV1>>>,
    pub compositor_space_transformer_v1:
        CloneCell<Option<Rc<JayHeadManagerExtCompositorSpaceTransformerV1>>>,
    pub physical_display_info_v1: CloneCell<Option<Rc<JayHeadManagerExtPhysicalDisplayInfoV1>>>,
    pub connector_info_v1: CloneCell<Option<Rc<JayHeadManagerExtConnectorInfoV1>>>,
    pub connector_settings_v1: CloneCell<Option<Rc<JayHeadManagerExtConnectorSettingsV1>>>,
}

impl JayHeadManagerV1 {
    pub fn announce(self: &Rc<Self>, connector: &ConnectorData) {
        if let Err(e) = self.try_announce(connector) {
            self.client.error(e);
        }
    }

    fn try_announce(self: &Rc<Self>, connector: &ConnectorData) -> Result<(), ClientError> {
        let common = Rc::new(HeadCommon {
            name: connector.head_name,
            removed: Cell::new(false),
        });
        let obj = Rc::new(JayHeadV1 {
            id: self.client.new_id()?,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            common: common.clone(),
        });
        track!(self.client, obj);
        self.client.add_server_obj(&obj);
        self.send_head_start(&obj, connector.head_name);
        macro_rules! ext {
            ($($field:ident,)*) => {{
                let head = super::Head {
                    mgr: self.clone(),
                    head: obj,
                    $(
                        $field: match self.$field.get() {
                            Some(f) => f.announce(connector, &common)?,
                            _ => None,
                        },
                    )*
                };
                self.send_head_complete();
                $(
                    if let Some(ext) = &head.$field {
                        ext.after_announce_wrapper(connector);
                    }
                )*
                head
            }};
        }
        let head = ext! {
            core_info_v1,
            compositor_space_info_v1,
            compositor_space_positioner_v1,
            compositor_space_transformer_v1,
            physical_display_info_v1,
            connector_info_v1,
            connector_settings_v1,
        };
        connector
            .head_managers
            .managers
            .set((self.client.id, self.id), Rc::new(head));
        Ok(())
    }

    pub fn schedule_done(self: &Rc<Self>) {
        if !self.done_scheduled.replace(true) {
            self.serial.fetch_add(1);
            self.client.state.head_managers_done.push(self.clone());
        }
    }

    pub fn send_done(&self) {
        self.client.event(Done {
            self_id: self.id,
            serial: self.serial.get(),
        });
    }

    pub fn send_stopped(&self) {
        self.client.event(Stopped { self_id: self.id });
    }

    fn send_extension(&self, name: u32, interface: &str, version: Version) {
        self.client.event(Extension {
            self_id: self.id,
            name,
            interface,
            version: version.0,
        });
    }

    fn send_extensions_done(&self) {
        self.client.event(ExtensionsDone { self_id: self.id });
    }

    fn send_head_start(&self, head: &JayHeadV1, name: HeadName) {
        self.client.event(HeadStart {
            self_id: self.id,
            head: head.id,
            name: name.0,
        });
    }

    fn send_head_complete(&self) {
        self.client.event(HeadComplete { self_id: self.id });
    }

    fn detach(&self, send_removed: bool) {
        let id = (self.client.id, self.id);
        for data in self.client.state.connectors.lock().values() {
            if let Some(head) = data.head_managers.managers.remove(&id) {
                if send_removed {
                    head.head.send_removed();
                }
            }
        }
        self.client.state.head_managers.remove(&id);
    }
}

impl JayHeadManagerV1RequestHandler for JayHeadManagerV1 {
    type Error = JayHeadManagerV1Error;

    fn destroy(
        &self,
        _req: crate::wire::jay_head_manager_v1::Destroy,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.common.assert_stopped()?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn bind_extension(&self, req: BindExtension<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.state.get() != HeadMgrState::Init {
            return Err(JayHeadManagerV1Error::AlreadyStarted);
        }
        let Some(ext) = HeadExtension::from_linear(req.name as usize) else {
            return Err(JayHeadManagerV1Error::UnknownExtension(req.name));
        };
        macro_rules! map {
            ($($field:ident = $ext:ident,)*) => {
                match ext {
                    $(
                        HeadExtension::$ext => {
                            if self.$field.is_some() {
                                return Err(JayHeadManagerV1Error::AlreadyBound(req.name));
                            }
                            let version = Version(req.version);
                            with_builtin_macros::with_eager_expansions! {
                                if version > #{concat_idents!(JayHeadManagerExt, $ext)}::VERSION {
                                    return Err(JayHeadManagerV1Error::UnsupportedVersion(req.name, req.version));
                                }
                                let obj = Rc::new(#{concat_idents!(JayHeadManagerExt, $ext)} {
                                    id: req.id.into(),
                                    client: self.client.clone(),
                                    tracker: Default::default(),
                                    version,
                                    common: self.common.clone(),
                                });
                            }
                            track!(self.client, obj);
                            self.client.add_client_obj(&obj)?;
                            self.$field.set(Some(obj));
                        }
                    )*
                }
            };
        }
        map! {
            core_info_v1 = CoreInfoV1,
            compositor_space_info_v1 = CompositorSpaceInfoV1,
            compositor_space_positioner_v1 = CompositorSpacePositionerV1,
            compositor_space_transformer_v1 = CompositorSpaceTransformerV1,
            physical_display_info_v1 = PhysicalDisplayInfoV1,
            connector_info_v1 = ConnectorInfoV1,
            connector_settings_v1 = ConnectorSettingsV1,
        }
        Ok(())
    }

    fn stop(&self, _req: Stop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.state.replace(HeadMgrState::Stopped) == HeadMgrState::Stopped {
            return Ok(());
        }
        self.detach(true);
        self.serial.fetch_add(1);
        self.send_done();
        self.send_stopped();
        Ok(())
    }

    fn start(&self, _req: Start, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.common.state.get() != HeadMgrState::Init {
            return Err(JayHeadManagerV1Error::AlreadyStarted);
        }
        self.common.state.set(HeadMgrState::Started);
        self.client
            .state
            .head_managers
            .set((self.client.id, self.id), slf.clone());
        for connector in self.client.state.connectors.lock().values() {
            slf.announce(connector);
        }
        slf.schedule_done();
        Ok(())
    }

    fn create_transaction(
        &self,
        req: CreateTransaction,
        slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let obj = Rc::new(JayHeadTransactionV1 {
            id: req.transaction,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            manager: slf.clone(),
            tran: Default::default(),
            serial: req.serial,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }
}

object_base! {
    self = JayHeadManagerV1;
    version = self.version;
}

impl Object for JayHeadManagerV1 {
    fn break_loops(&self) {
        self.detach(false);
    }
}

dedicated_add_obj!(JayHeadManagerV1, JayHeadManagerV1Id, jay_head_managers);

#[derive(Debug, Error)]
pub enum JayHeadManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    Common(#[from] HeadCommonError),
    #[error("Manager was already started")]
    AlreadyStarted,
    #[error("There is no extension with name {}", .0)]
    UnknownExtension(u32),
    #[error("The extension with name {} is already bound", .0)]
    AlreadyBound(u32),
    #[error("The extension with name {} does not support version {}", .0, .1)]
    UnsupportedVersion(u32, u32),
}
efrom!(JayHeadManagerV1Error, ClientError);
