use {
    super::zwlr_output_head_v1::{
        ADAPTIVE_SYNC_STATE_DISABLED, ADAPTIVE_SYNC_STATE_ENABLED, ZwlrOutputHeadV1,
    },
    crate::{
        backend::Mode,
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        utils::opt::Opt,
        wire::{ZwlrOutputConfigurationHeadV1Id, zwlr_output_configuration_head_v1::*},
    },
    jay_config::video::Transform,
    std::{cell::RefCell, rc::Rc},
    thiserror::Error,
};

pub struct ZwlrOutputConfigurationHeadV1 {
    pub(super) id: ZwlrOutputConfigurationHeadV1Id,
    pub(super) version: Version,
    pub(super) client: Rc<Client>,
    pub(super) head: Rc<Opt<ZwlrOutputHeadV1>>,
    pub(super) config: Rc<RefCell<OutputConfig>>,
    pub(super) tracker: Tracker<Self>,
}

#[derive(Default)]
pub struct OutputConfig {
    pub(super) transform: Option<Transform>,
    pub(super) scale: Option<f64>,
    pub(super) vrr_enabled: Option<bool>,
    pub(super) x: Option<i32>,
    pub(super) y: Option<i32>,
    pub(super) mode: Option<Mode>,
    pub(super) custom_mode: Option<Mode>,
}

impl ZwlrOutputConfigurationHeadV1RequestHandler for ZwlrOutputConfigurationHeadV1 {
    type Error = ZwlrOutputConfigurationHeadV1Error;

    fn set_mode(&self, req: SetMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.config.borrow().mode.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }

        let mode = self.client.lookup(req.mode);
        if let Ok(mode) = mode {
            if let (Some(mode_head), Some(configuration_head)) = (mode.head.get(), self.head.get())
            {
                if mode_head.id != configuration_head.id {
                    return Err(Self::Error::InvalidMode);
                }
                self.config.borrow_mut().mode = Some(Mode {
                    width: mode.width.get(),
                    height: mode.height.get(),
                    refresh_rate_millihz: mode.refresh.get().unwrap_or_default() as u32,
                });
            }
        }
        Ok(())
    }

    fn set_custom_mode(&self, req: SetCustomMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.config.borrow().custom_mode.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        if req.refresh <= 0 {
            return Err(ZwlrOutputConfigurationHeadV1Error::InvalidCustomMode);
        }
        self.config.borrow_mut().custom_mode = Some(Mode {
            width: req.width,
            height: req.height,
            refresh_rate_millihz: req.refresh as u32,
        });
        Ok(())
    }

    fn set_position(&self, req: SetPosition, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.config.borrow().x.is_some() || self.config.borrow().y.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        self.config.borrow_mut().x = Some(req.x);
        self.config.borrow_mut().y = Some(req.y);
        Ok(())
    }

    fn set_transform(&self, req: SetTransform, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.config.borrow().transform.is_some() {
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
        self.config.borrow_mut().transform = Some(transform);
        Ok(())
    }

    fn set_scale(&self, req: SetScale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.config.borrow().scale.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }

        if req.scale <= 0 {
            Err(ZwlrOutputConfigurationHeadV1Error::InvalidScale)
        } else {
            self.config.borrow_mut().scale = Some(req.scale.to_f64());
            Ok(())
        }
    }

    fn set_adaptive_sync(&self, req: SetAdaptiveSync, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.config.borrow().vrr_enabled.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }

        let state = match req.state {
            ADAPTIVE_SYNC_STATE_DISABLED => false,
            ADAPTIVE_SYNC_STATE_ENABLED => true,
            _ => return Err(ZwlrOutputConfigurationHeadV1Error::InvalidAdaptiveSyncState),
        };
        self.config.borrow_mut().vrr_enabled = Some(state);
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
