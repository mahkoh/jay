use {
    crate::{
        backend::Mode,
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::wlr_output_manager::zwlr_output_head_v1::{
            ADAPTIVE_SYNC_STATE_DISABLED, ADAPTIVE_SYNC_STATE_ENABLED, WlrOutputHeadId,
        },
        leaks::Tracker,
        object::{Object, Version},
        scale::Scale,
        tree::VrrMode,
        utils::transform_ext::TransformExt,
        wire::{ZwlrOutputConfigurationHeadV1Id, zwlr_output_configuration_head_v1::*},
    },
    jay_config::video::Transform,
    std::{cell::RefCell, rc::Rc},
    thiserror::Error,
};

pub struct ZwlrOutputConfigurationHeadV1 {
    pub(super) id: ZwlrOutputConfigurationHeadV1Id,
    pub(super) head_id: WlrOutputHeadId,
    pub(super) version: Version,
    pub(super) client: Rc<Client>,
    pub(super) config: RefCell<OutputConfig>,
    pub(super) tracker: Tracker<Self>,
}

#[derive(Default, Copy, Clone)]
pub struct OutputConfig {
    pub(super) transform: Option<Transform>,
    pub(super) scale: Option<Scale>,
    pub(super) vrr_mode: Option<&'static VrrMode>,
    pub(super) pos: Option<(i32, i32)>,
    pub(super) mode: Option<Mode>,
}

impl ZwlrOutputConfigurationHeadV1RequestHandler for ZwlrOutputConfigurationHeadV1 {
    type Error = ZwlrOutputConfigurationHeadV1Error;

    fn set_mode(&self, req: SetMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let config = &mut *self.config.borrow_mut();
        if config.mode.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        let mode = self.client.lookup(req.mode)?;
        if self.head_id != mode.head_id {
            return Err(ZwlrOutputConfigurationHeadV1Error::InvalidMode);
        }
        config.mode = Some(mode.mode);
        Ok(())
    }

    fn set_custom_mode(&self, req: SetCustomMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let config = &mut *self.config.borrow_mut();
        if config.mode.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        config.mode = Some(Mode {
            width: req.width,
            height: req.height,
            refresh_rate_millihz: req.refresh as u32,
        });
        Ok(())
    }

    fn set_position(&self, req: SetPosition, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let config = &mut *self.config.borrow_mut();
        if config.pos.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        config.pos = Some((req.x, req.y));
        Ok(())
    }

    fn set_transform(&self, req: SetTransform, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let config = &mut *self.config.borrow_mut();
        if config.transform.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        let Some(transform) = Transform::from_wl(req.transform) else {
            return Err(ZwlrOutputConfigurationHeadV1Error::InvalidTransform(
                req.transform,
            ));
        };
        config.transform = Some(transform);
        Ok(())
    }

    fn set_scale(&self, req: SetScale, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let config = &mut *self.config.borrow_mut();
        if config.scale.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        if req.scale <= 0 {
            return Err(ZwlrOutputConfigurationHeadV1Error::InvalidScale(req.scale));
        }
        config.scale = Some(Scale::from_f64(req.scale.to_f64()));
        Ok(())
    }

    fn set_adaptive_sync(&self, req: SetAdaptiveSync, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let config = &mut *self.config.borrow_mut();
        if config.vrr_mode.is_some() {
            return Err(ZwlrOutputConfigurationHeadV1Error::AlreadySet);
        }
        let state = match req.state {
            ADAPTIVE_SYNC_STATE_DISABLED => VrrMode::NEVER,
            ADAPTIVE_SYNC_STATE_ENABLED => VrrMode::ALWAYS,
            _ => {
                return Err(
                    ZwlrOutputConfigurationHeadV1Error::InvalidAdaptiveSyncState(req.state),
                );
            }
        };
        config.vrr_mode = Some(state);
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
    #[error("Property has already been set")]
    AlreadySet,
    #[error("Mode doesn't belong to head")]
    InvalidMode,
    #[error("Unknown transform {0}")]
    InvalidTransform(i32),
    #[error("Invalid scale {0}")]
    InvalidScale(Fixed),
    #[error("Invalid adaptive sync state {0}")]
    InvalidAdaptiveSyncState(u32),
}
efrom!(ZwlrOutputConfigurationHeadV1Error, ClientError);
