use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        video::drm::sync_obj::SyncObjPoint,
        wire::{wp_linux_drm_syncobj_surface_v1::*, WpLinuxDrmSyncobjSurfaceV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpLinuxDrmSyncobjSurfaceV1 {
    id: WpLinuxDrmSyncobjSurfaceV1Id,
    client: Rc<Client>,
    surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
}

impl WpLinuxDrmSyncobjSurfaceV1 {
    pub fn new(
        id: WpLinuxDrmSyncobjSurfaceV1Id,
        client: &Rc<Client>,
        surface: &Rc<WlSurface>,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            surface: surface.clone(),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WpLinuxDrmSyncobjSurfaceV1Error> {
        if self.surface.sync_obj_surface.is_some() {
            return Err(WpLinuxDrmSyncobjSurfaceV1Error::Exists);
        }
        self.surface.sync_obj_surface.set(Some(self.clone()));
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WpLinuxDrmSyncobjSurfaceV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.surface.sync_obj_surface.take();
        let pending = &mut *self.surface.pending.borrow_mut();
        pending.release_point.take();
        pending.acquire_point.take();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_acquire_point(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WpLinuxDrmSyncobjSurfaceV1Error> {
        let req: SetAcquirePoint = self.client.parse(self, parser)?;
        let point = point(req.point_hi, req.point_lo);
        let timeline = self.client.lookup(req.timeline)?;
        self.surface.pending.borrow_mut().acquire_point = Some((timeline.sync_obj.clone(), point));
        Ok(())
    }

    fn set_release_point(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WpLinuxDrmSyncobjSurfaceV1Error> {
        let req: SetReleasePoint = self.client.parse(self, parser)?;
        let point = point(req.point_hi, req.point_lo);
        let timeline = self.client.lookup(req.timeline)?;
        self.surface.pending.borrow_mut().release_point = Some((timeline.sync_obj.clone(), point));
        Ok(())
    }
}

fn point(hi: u32, lo: u32) -> SyncObjPoint {
    SyncObjPoint((hi as u64) << 32 | (lo as u64))
}

object_base! {
    self = WpLinuxDrmSyncobjSurfaceV1;

    DESTROY => destroy,
    SET_ACQUIRE_POINT => set_acquire_point,
    SET_RELEASE_POINT => set_release_point,
}

impl Object for WpLinuxDrmSyncobjSurfaceV1 {}

simple_add_obj!(WpLinuxDrmSyncobjSurfaceV1);

#[derive(Debug, Error)]
pub enum WpLinuxDrmSyncobjSurfaceV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a syncobj extension attached")]
    Exists,
}
efrom!(WpLinuxDrmSyncobjSurfaceV1Error, MsgParserError);
efrom!(WpLinuxDrmSyncobjSurfaceV1Error, ClientError);
