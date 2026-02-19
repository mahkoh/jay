use {
    crate::{
        client::{Client, ClientError},
        gfx_api::SyncFile,
        ifs::wl_surface::{WlSurface, jay_sync_file_release::JaySyncFileRelease},
        leaks::Tracker,
        object::{Object, Version},
        wire::{JaySyncFileSurfaceId, jay_sync_file_surface::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JaySyncFileSurface {
    pub id: JaySyncFileSurfaceId,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JaySyncFileSurface {
    pub fn new(id: JaySyncFileSurfaceId, version: Version, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
            client: surface.client.clone(),
            surface: surface.clone(),
            tracker: Default::default(),
            version,
        }
    }
}

impl JaySyncFileSurfaceRequestHandler for JaySyncFileSurface {
    type Error = JaySyncFileSurfaceError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_acquire_immediate(
        &self,
        _req: SetAcquireImmediate,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.surface.pending.borrow_mut().sync_file_acquire = Some(None);
        Ok(())
    }

    fn set_acquire_async(&self, req: SetAcquireAsync, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.pending.borrow_mut().sync_file_acquire = Some(Some(SyncFile(req.sync_file)));
        Ok(())
    }

    fn get_release(&self, req: GetRelease, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(JaySyncFileRelease::new(
            &self.client,
            req.release,
            self.version,
        ));
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        let pending = &mut *self.surface.pending.borrow_mut();
        if pending.sync_file_release.is_some() {
            return Err(JaySyncFileSurfaceError::HasRelease);
        }
        pending.sync_file_release = Some(obj);
        Ok(())
    }
}

object_base! {
    self = JaySyncFileSurface;
    version = self.version;
}

impl Object for JaySyncFileSurface {}

simple_add_obj!(JaySyncFileSurface);

#[derive(Debug, Error)]
pub enum JaySyncFileSurfaceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The content update already has a release object")]
    HasRelease,
}
efrom!(JaySyncFileSurfaceError, ClientError);
