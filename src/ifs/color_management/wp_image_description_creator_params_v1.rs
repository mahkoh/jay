use {
    crate::{
        client::{Client, ClientError},
        ifs::color_management::{
            consts::{PRIMARIES_SRGB, TRANSFER_FUNCTION_SRGB},
            wp_image_description_v1::WpImageDescriptionV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            WpImageDescriptionCreatorParamsV1Id,
            wp_image_description_creator_params_v1::{
                Create, SetLuminances, SetMasteringDisplayPrimaries, SetMasteringLuminance,
                SetMaxCll, SetMaxFall, SetPrimaries, SetPrimariesNamed, SetTfNamed, SetTfPower,
                WpImageDescriptionCreatorParamsV1RequestHandler,
            },
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpImageDescriptionCreatorParamsV1 {
    pub id: WpImageDescriptionCreatorParamsV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl WpImageDescriptionCreatorParamsV1RequestHandler for WpImageDescriptionCreatorParamsV1 {
    type Error = WpImageDescriptionCreatorParamsV1Error;

    fn create(&self, req: Create, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(WpImageDescriptionV1 {
            id: req.image_description,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_ready(0);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_tf_named(&self, req: SetTfNamed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.tf != TRANSFER_FUNCTION_SRGB {
            return Err(WpImageDescriptionCreatorParamsV1Error::UnsupportedTf(
                req.tf,
            ));
        }
        Ok(())
    }

    fn set_tf_power(&self, _req: SetTfPower, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Err(WpImageDescriptionCreatorParamsV1Error::SetTfPowerNotSupported)
    }

    fn set_primaries_named(
        &self,
        req: SetPrimariesNamed,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if req.primaries != PRIMARIES_SRGB {
            return Err(
                WpImageDescriptionCreatorParamsV1Error::UnsupportedPrimaries(req.primaries),
            );
        }
        Ok(())
    }

    fn set_primaries(&self, _req: SetPrimaries, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Err(WpImageDescriptionCreatorParamsV1Error::SetPrimariesNotSupported)
    }

    fn set_luminances(&self, _req: SetLuminances, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Err(WpImageDescriptionCreatorParamsV1Error::SetLuminancesNotSupported)
    }

    fn set_mastering_display_primaries(
        &self,
        _req: SetMasteringDisplayPrimaries,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Err(WpImageDescriptionCreatorParamsV1Error::SetMasteringDisplayPrimariesNotSupported)
    }

    fn set_mastering_luminance(
        &self,
        _req: SetMasteringLuminance,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Err(WpImageDescriptionCreatorParamsV1Error::SetMasteringLuminanceNotSupported)
    }

    fn set_max_cll(&self, _req: SetMaxCll, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_max_fall(&self, _req: SetMaxFall, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

object_base! {
    self = WpImageDescriptionCreatorParamsV1;
    version = self.version;
}

impl Object for WpImageDescriptionCreatorParamsV1 {}

simple_add_obj!(WpImageDescriptionCreatorParamsV1);

#[derive(Debug, Error)]
pub enum WpImageDescriptionCreatorParamsV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("set_mastering_luminance is not supported")]
    SetMasteringLuminanceNotSupported,
    #[error("set_mastering_display_primaries is not supported")]
    SetMasteringDisplayPrimariesNotSupported,
    #[error("set_luminances is not supported")]
    SetLuminancesNotSupported,
    #[error("set_primaries is not supported")]
    SetPrimariesNotSupported,
    #[error("{} is not a supported named primary", .0)]
    UnsupportedPrimaries(u32),
    #[error("set_tf_power is not supported")]
    SetTfPowerNotSupported,
    #[error("{} is not a supported named transfer function", .0)]
    UnsupportedTf(u32),
}
efrom!(WpImageDescriptionCreatorParamsV1Error, ClientError);
