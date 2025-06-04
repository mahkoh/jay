use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            WpCommitTimerV1Id,
            wp_commit_timer_v1::{Destroy, SetTimestamp, WpCommitTimerV1RequestHandler},
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpCommitTimerV1 {
    pub id: WpCommitTimerV1Id,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpCommitTimerV1 {
    pub fn new(id: WpCommitTimerV1Id, version: Version, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
            client: surface.client.clone(),
            surface: surface.clone(),
            tracker: Default::default(),
            version,
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WpCommitTimerV1Error> {
        if self.surface.commit_timer.is_some() {
            return Err(WpCommitTimerV1Error::Exists);
        }
        self.surface.commit_timer.set(Some(self.clone()));
        Ok(())
    }
}

impl WpCommitTimerV1RequestHandler for WpCommitTimerV1 {
    type Error = WpCommitTimerV1Error;

    fn set_timestamp(&self, req: SetTimestamp, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.tv_nsec >= 1_000_000_000 {
            return Err(WpCommitTimerV1Error::InvalidNsec);
        }
        let nsec = req
            .tv_sec
            .checked_mul(1_000_000_000)
            .and_then(|n| n.checked_add(req.tv_nsec as u64));
        let Some(nsec) = nsec else {
            return Err(WpCommitTimerV1Error::Overflow);
        };
        let pending = &mut *self.surface.pending.borrow_mut();
        if pending.commit_time.is_some() {
            return Err(WpCommitTimerV1Error::TimestampExists);
        }
        pending.commit_time = Some(nsec);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.commit_timer.take();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpCommitTimerV1;
    version = self.version;
}

impl Object for WpCommitTimerV1 {}

simple_add_obj!(WpCommitTimerV1);

#[derive(Debug, Error)]
pub enum WpCommitTimerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a commit timer extension attached")]
    Exists,
    #[error("The tv_nsec is larger than 999_999_999")]
    InvalidNsec,
    #[error("The timestamp overflowed")]
    Overflow,
    #[error("The commit already has a timestamp")]
    TimestampExists,
}
efrom!(WpCommitTimerV1Error, ClientError);
