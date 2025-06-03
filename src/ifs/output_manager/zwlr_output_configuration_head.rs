use {
    super::zwlr_output_head_v1::{
        ADAPTIVE_SYNC_STATE_DISABLED, ADAPTIVE_SYNC_STATE_ENABLED, ZwlrOutputHeadV1,
    },
    crate::{
        backend::Mode,
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        utils::{cell_ext::CellExt, opt::Opt},
        wire::{ZwlrOutputConfigurationHeadV1Id, zwlr_output_configuration_head_v1::*},
    },
    jay_config::video::Transform,
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwlrOutputConfigurationHeadV1 {
    pub id: ZwlrOutputConfigurationHeadV1Id,
    pub version: Version,
    pub client: Rc<Client>,
    pub head: Rc<Opt<ZwlrOutputHeadV1>>,
    pub transform: Cell<Option<Transform>>,
    pub scale: Cell<Option<f64>>,
    pub vrr_enabled: Cell<Option<bool>>,
    pub x: Cell<Option<i32>>,
    pub y: Cell<Option<i32>>,
    pub mode: Cell<Option<Mode>>,
    pub custom_mode: Cell<Option<Mode>>,
    pub tracker: Tracker<Self>,
}

impl ZwlrOutputConfigurationHeadV1 {}

impl ZwlrOutputConfigurationHeadV1RequestHandler for ZwlrOutputConfigurationHeadV1 {
    type Error = ZwlrOutputConfigurationHeadV1Error;

    fn set_mode(&self, req: SetMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.mode.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }

        let mode = self.client.lookup(req.mode);
        if let Ok(mode) = mode {
            if mode.head.get().unwrap().id != self.head.get().unwrap().id {
                return Err(Self::Error::InvalidMode);
            }
            self.mode.set(Some(Mode {
                width: mode.width.get(),
                height: mode.height.get(),
                refresh_rate_millihz: mode.refresh.get().unwrap_or_default() as u32,
            }));
        }
        Ok(())
    }

    fn set_custom_mode(&self, req: SetCustomMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.custom_mode.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        if req.refresh <= 0 {
            return Err(ZwlrOutputConfigurationHeadV1Error::InvalidCustomMode);
        }
        self.custom_mode.set(Some(Mode {
            width: req.width,
            height: req.height,
            refresh_rate_millihz: req.refresh as u32,
        }));
        Ok(())
    }

    fn set_position(&self, req: SetPosition, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.x.is_some() || self.y.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        self.x.set(Some(req.x));
        self.y.set(Some(req.y));
        Ok(())
    }

    fn set_transform(&self, req: SetTransform, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.transform.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        let transform = match req.transform {
            0 => Transform::None,
            1 => Transform::Rotate90,
            2 => Transform::Rotate180,
            3 => Transform::Rotate270,
            4 => Transform::Flip,
            5 => Transform::FlipRotate90,
            6 => Transform::FlipRotate180,
            7 => Transform::FlipRotate270,
            _ => return Err(ZwlrOutputConfigurationHeadV1Error::InvalidTransform),
        };
        self.transform.set(Some(transform));
        Ok(())
    }

    fn set_scale(&self, req: SetScale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.scale.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }

        if req.scale <= 0 {
            Err(ZwlrOutputConfigurationHeadV1Error::InvalidScale)
        } else {
            self.scale.set(Some(req.scale.to_f64()));
            Ok(())
        }
    }

    fn set_adaptive_sync(&self, req: SetAdaptiveSync, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.vrr_enabled.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }

        let state = match req.state {
            ADAPTIVE_SYNC_STATE_DISABLED => false,
            ADAPTIVE_SYNC_STATE_ENABLED => true,
            _ => return Err(ZwlrOutputConfigurationHeadV1Error::InvalidAdaptiveSyncState),
        };
        self.vrr_enabled.set(Some(state));
        Ok(())
    }
}

object_base! {
    self = ZwlrOutputConfigurationHeadV1;
    version = self.version;
}

impl Object for ZwlrOutputConfigurationHeadV1 {}

simple_add_obj!(ZwlrOutputConfigurationHeadV1);

#[derive(Debug, Error)]
pub enum ZwlrOutputConfigurationHeadV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("property has already been set")]
    AlreadySet,
    #[error("mode doesn't belong to head")]
    InvalidMode,
    #[error("mode is invalid")]
    InvalidCustomMode,
    #[error("transform value outside enum")]
    InvalidTransform,
    #[error("scale negative or zero")]
    InvalidScale,
    #[error("invalid enum value used in the set_adaptive_sync request")]
    InvalidAdaptiveSyncState,
}
efrom!(ZwlrOutputConfigurationHeadV1Error, ClientError);
