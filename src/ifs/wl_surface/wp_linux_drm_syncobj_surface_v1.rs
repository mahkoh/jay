use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::{SyncobjRelease, WlSurface},
        leaks::Tracker,
        object::{Object, Version},
        video::drm::syncobj::SyncobjPoint,
        wire::{WpLinuxDrmSyncobjSurfaceV1Id, wp_linux_drm_syncobj_surface_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpLinuxDrmSyncobjSurfaceV1 {
    id: WpLinuxDrmSyncobjSurfaceV1Id,
    client: Rc<Client>,
    surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    version: Version,
}

impl WpLinuxDrmSyncobjSurfaceV1 {
    pub fn new(
        id: WpLinuxDrmSyncobjSurfaceV1Id,
        client: &Rc<Client>,
        surface: &Rc<WlSurface>,
        version: Version,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            surface: surface.clone(),
            version,
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WpLinuxDrmSyncobjSurfaceV1Error> {
        if self.surface.syncobj_surface.is_some() {
            return Err(WpLinuxDrmSyncobjSurfaceV1Error::Exists);
        }
        self.surface.syncobj_surface.set(Some(self.clone()));
        Ok(())
    }
}

impl WpLinuxDrmSyncobjSurfaceV1RequestHandler for WpLinuxDrmSyncobjSurfaceV1 {
    type Error = WpLinuxDrmSyncobjSurfaceV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.syncobj_surface.take();
        let pending = &mut *self.surface.pending.borrow_mut();
        pending.release_point.take();
        pending.acquire_point.take();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_acquire_point(&self, req: SetAcquirePoint, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let point = SyncobjPoint(req.point);
        let timeline = self.client.lookup(req.timeline)?;
        self.surface.pending.borrow_mut().acquire_point = Some((timeline.syncobj.clone(), point));
        Ok(())
    }

    fn set_release_point(&self, req: SetReleasePoint, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let point = SyncobjPoint(req.point);
        let timeline = self.client.lookup(req.timeline)?;
        self.surface.pending.borrow_mut().release_point = Some(SyncobjRelease {
            state: self.client.state.clone(),
            committed: false,
            syncobj: Some(timeline.syncobj.clone()),
            point,
        });
        Ok(())
    }
}

object_base! {
    self = WpLinuxDrmSyncobjSurfaceV1;
    version = self.version;
}

impl Object for WpLinuxDrmSyncobjSurfaceV1 {}

simple_add_obj!(WpLinuxDrmSyncobjSurfaceV1);

#[derive(Debug, Error)]
pub enum WpLinuxDrmSyncobjSurfaceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a syncobj extension attached")]
    Exists,
}
efrom!(WpLinuxDrmSyncobjSurfaceV1Error, ClientError);
