use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            wl_surface::wp_linux_drm_syncobj_surface_v1::{
                WpLinuxDrmSyncobjSurfaceV1, WpLinuxDrmSyncobjSurfaceV1Error,
            },
            wp_linux_drm_syncobj_timeline_v1::WpLinuxDrmSyncobjTimelineV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::buffd::{MsgParser, MsgParserError},
        video::drm::sync_obj::SyncObj,
        wire::{wp_linux_drm_syncobj_manager_v1::*, WpLinuxDrmSyncobjManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpLinuxDrmSyncobjManagerV1Global {
    pub name: GlobalName,
}

pub struct WpLinuxDrmSyncobjManagerV1 {
    pub id: WpLinuxDrmSyncobjManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl WpLinuxDrmSyncobjManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpLinuxDrmSyncobjManagerV1Id,
        client: &Rc<Client>,
        _version: Version,
    ) -> Result<(), WpLinuxDrmSyncobjManagerV1Error> {
        let obj = Rc::new(WpLinuxDrmSyncobjManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    WpLinuxDrmSyncobjManagerV1Global,
    WpLinuxDrmSyncobjManagerV1,
    WpLinuxDrmSyncobjManagerV1Error
);

impl Global for WpLinuxDrmSyncobjManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpLinuxDrmSyncobjManagerV1Global);

impl WpLinuxDrmSyncobjManagerV1 {
    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), WpLinuxDrmSyncobjManagerV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_surface(&self, msg: MsgParser<'_, '_>) -> Result<(), WpLinuxDrmSyncobjManagerV1Error> {
        let req: GetSurface = self.client.parse(self, msg)?;
        let surface = self.client.lookup(req.surface)?;
        let sync = Rc::new(WpLinuxDrmSyncobjSurfaceV1::new(
            req.id,
            &self.client,
            &surface,
        ));
        track!(self.client, sync);
        sync.install()?;
        self.client.add_client_obj(&sync)?;
        Ok(())
    }

    fn import_timeline(
        &self,
        msg: MsgParser<'_, '_>,
    ) -> Result<(), WpLinuxDrmSyncobjManagerV1Error> {
        let req: ImportTimeline = self.client.parse(self, msg)?;
        let sync_obj = Rc::new(SyncObj::new(&req.fd));
        let sync = Rc::new(WpLinuxDrmSyncobjTimelineV1::new(
            req.id,
            &self.client,
            &sync_obj,
        ));
        self.client.add_client_obj(&sync)?;
        Ok(())
    }
}

object_base! {
    self = WpLinuxDrmSyncobjManagerV1;

    DESTROY => destroy,
    GET_SURFACE => get_surface,
    IMPORT_TIMELINE => import_timeline,
}

impl Object for WpLinuxDrmSyncobjManagerV1 {}

simple_add_obj!(WpLinuxDrmSyncobjManagerV1);

#[derive(Debug, Error)]
pub enum WpLinuxDrmSyncobjManagerV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WpLinuxDrmSyncobjSurfaceV1Error(#[from] WpLinuxDrmSyncobjSurfaceV1Error),
}
efrom!(WpLinuxDrmSyncobjManagerV1Error, MsgParserError);
efrom!(WpLinuxDrmSyncobjManagerV1Error, ClientError);
