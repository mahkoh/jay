use {
    crate::{
        client::{Client, ClientError},
        cmm::{
            cmm_luminance::{Luminance, TargetLuminance},
            cmm_primaries::{NamedPrimaries, Primaries},
            cmm_transfer_function::TransferFunction,
        },
        ifs::color_management::{
            MIN_LUM_MUL_INV, PRIMARIES_MUL_INV,
            consts::{
                PRIMARIES_ADOBE_RGB, PRIMARIES_BT2020, PRIMARIES_CIE1931_XYZ, PRIMARIES_DCI_P3,
                PRIMARIES_DISPLAY_P3, PRIMARIES_GENERIC_FILM, PRIMARIES_NTSC, PRIMARIES_PAL,
                PRIMARIES_PAL_M, PRIMARIES_SRGB, TRANSFER_FUNCTION_BT1886,
                TRANSFER_FUNCTION_EXT_LINEAR, TRANSFER_FUNCTION_EXT_SRGB,
                TRANSFER_FUNCTION_GAMMA22, TRANSFER_FUNCTION_GAMMA28, TRANSFER_FUNCTION_LOG_100,
                TRANSFER_FUNCTION_LOG_316, TRANSFER_FUNCTION_SRGB, TRANSFER_FUNCTION_ST240,
                TRANSFER_FUNCTION_ST428, TRANSFER_FUNCTION_ST2084_PQ,
            },
            wp_image_description_v1::WpImageDescriptionV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::ordered_float::F64,
        wire::{
            WpImageDescriptionCreatorParamsV1Id,
            wp_image_description_creator_params_v1::{
                Create, SetLuminances, SetMasteringDisplayPrimaries, SetMasteringLuminance,
                SetMaxCll, SetMaxFall, SetPrimaries, SetPrimariesNamed, SetTfNamed, SetTfPower,
                WpImageDescriptionCreatorParamsV1RequestHandler,
            },
        },
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct WpImageDescriptionCreatorParamsV1 {
    pub id: WpImageDescriptionCreatorParamsV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub tf: Cell<Option<TransferFunction>>,
    pub primaries: Cell<Option<(Option<NamedPrimaries>, Primaries)>>,
    pub luminance: Cell<Option<Luminance>>,
    pub mastering_primaries: Cell<Option<Primaries>>,
    pub mastering_luminance: Cell<Option<TargetLuminance>>,
    pub max_cll: Cell<Option<F64>>,
    pub max_fall: Cell<Option<F64>>,
}

impl WpImageDescriptionCreatorParamsV1RequestHandler for WpImageDescriptionCreatorParamsV1 {
    type Error = WpImageDescriptionCreatorParamsV1Error;

    fn create(&self, req: Create, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(transfer_function) = self.tf.get() else {
            return Err(WpImageDescriptionCreatorParamsV1Error::TfNotSet);
        };
        let Some((named_primaries, primaries)) = self.primaries.get() else {
            return Err(WpImageDescriptionCreatorParamsV1Error::PrimariesNotSet);
        };
        let default_luminance = match transfer_function {
            TransferFunction::Bt1886 => Luminance::BT1886,
            TransferFunction::St2084Pq => Luminance::ST2084_PQ,
            _ => Luminance::SRGB,
        };
        let mut luminance = self.luminance.get().unwrap_or(default_luminance);
        if transfer_function == TransferFunction::St2084Pq {
            luminance.max.0 = luminance.min.0 + 10_000.0;
        }
        if luminance.max.0 <= luminance.min.0 || luminance.white.0 <= luminance.min.0 {
            return Err(WpImageDescriptionCreatorParamsV1Error::MinLuminanceTooLow);
        }
        let target_primaries = self.mastering_primaries.get().unwrap_or(primaries);
        let target_luminance = self
            .mastering_luminance
            .get()
            .unwrap_or(luminance.to_target());
        let description = self.client.state.color_manager.get_description(
            named_primaries,
            primaries,
            luminance,
            transfer_function,
            target_primaries,
            target_luminance,
            self.max_cll.get(),
            self.max_fall.get(),
        );
        let obj = Rc::new(WpImageDescriptionV1 {
            id: req.image_description,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            description,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_ready();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_tf_named(&self, req: SetTfNamed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tf = match req.tf {
            TRANSFER_FUNCTION_BT1886 => TransferFunction::Bt1886,
            TRANSFER_FUNCTION_GAMMA22 => TransferFunction::Gamma22,
            TRANSFER_FUNCTION_GAMMA28 => TransferFunction::Gamma28,
            TRANSFER_FUNCTION_ST240 => TransferFunction::St240,
            TRANSFER_FUNCTION_EXT_LINEAR => TransferFunction::Linear,
            TRANSFER_FUNCTION_LOG_100 => TransferFunction::Log100,
            TRANSFER_FUNCTION_LOG_316 => TransferFunction::Log316,
            TRANSFER_FUNCTION_SRGB => TransferFunction::Srgb,
            TRANSFER_FUNCTION_EXT_SRGB => TransferFunction::ExtSrgb,
            TRANSFER_FUNCTION_ST2084_PQ => TransferFunction::St2084Pq,
            TRANSFER_FUNCTION_ST428 => TransferFunction::St428,
            _ => {
                return Err(WpImageDescriptionCreatorParamsV1Error::UnsupportedTf(
                    req.tf,
                ));
            }
        };
        if self.tf.replace(Some(tf)).is_some() {
            return Err(WpImageDescriptionCreatorParamsV1Error::TfAlreadySet);
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
        let primaries = match req.primaries {
            PRIMARIES_SRGB => NamedPrimaries::Srgb,
            PRIMARIES_PAL_M => NamedPrimaries::PalM,
            PRIMARIES_PAL => NamedPrimaries::Pal,
            PRIMARIES_NTSC => NamedPrimaries::Ntsc,
            PRIMARIES_GENERIC_FILM => NamedPrimaries::GenericFilm,
            PRIMARIES_BT2020 => NamedPrimaries::Bt2020,
            PRIMARIES_CIE1931_XYZ => NamedPrimaries::Cie1931Xyz,
            PRIMARIES_DCI_P3 => NamedPrimaries::DciP3,
            PRIMARIES_DISPLAY_P3 => NamedPrimaries::DisplayP3,
            PRIMARIES_ADOBE_RGB => NamedPrimaries::AdobeRgb,
            _ => {
                return Err(
                    WpImageDescriptionCreatorParamsV1Error::UnsupportedPrimaries(req.primaries),
                );
            }
        };
        let primaries = (Some(primaries), primaries.primaries());
        if self.primaries.replace(Some(primaries)).is_some() {
            return Err(WpImageDescriptionCreatorParamsV1Error::PrimariesAlreadySet);
        }
        Ok(())
    }

    fn set_primaries(&self, req: SetPrimaries, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let map = |n: i32| F64(n as f64 * PRIMARIES_MUL_INV);
        let primaries = Primaries {
            r: (map(req.r_x), map(req.r_y)),
            g: (map(req.g_x), map(req.g_y)),
            b: (map(req.b_x), map(req.b_y)),
            wp: (map(req.w_x), map(req.w_y)),
        };
        let primaries = (None, primaries);
        if self.primaries.replace(Some(primaries)).is_some() {
            return Err(WpImageDescriptionCreatorParamsV1Error::PrimariesAlreadySet);
        }
        Ok(())
    }

    fn set_luminances(&self, req: SetLuminances, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let luminance = Luminance {
            min: F64(req.min_lum as f64 * MIN_LUM_MUL_INV),
            max: F64(req.max_lum as f64),
            white: F64(req.reference_lum as f64),
        };
        if self.luminance.replace(Some(luminance)).is_some() {
            return Err(WpImageDescriptionCreatorParamsV1Error::LuminancesAlreadySet);
        }
        Ok(())
    }

    fn set_mastering_display_primaries(
        &self,
        req: SetMasteringDisplayPrimaries,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let map = |n: i32| F64(n as f64 * PRIMARIES_MUL_INV);
        let primaries = Primaries {
            r: (map(req.r_x), map(req.r_y)),
            g: (map(req.g_x), map(req.g_y)),
            b: (map(req.b_x), map(req.b_y)),
            wp: (map(req.w_x), map(req.w_y)),
        };
        if self.mastering_primaries.replace(Some(primaries)).is_some() {
            return Err(WpImageDescriptionCreatorParamsV1Error::MasteringPrimariesAlreadySet);
        }
        Ok(())
    }

    fn set_mastering_luminance(
        &self,
        req: SetMasteringLuminance,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let luminance = TargetLuminance {
            min: F64(req.min_lum as f64 * MIN_LUM_MUL_INV),
            max: F64(req.max_lum as f64),
        };
        if luminance.max.0 <= luminance.min.0 {
            return Err(WpImageDescriptionCreatorParamsV1Error::MinMasteringLuminanceTooLow);
        }
        if self.mastering_luminance.replace(Some(luminance)).is_some() {
            return Err(WpImageDescriptionCreatorParamsV1Error::MasteringLuminancesAlreadySet);
        }
        Ok(())
    }

    fn set_max_cll(&self, req: SetMaxCll, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self
            .max_cll
            .replace(Some(F64(req.max_cll as f64)))
            .is_some()
        {
            return Err(WpImageDescriptionCreatorParamsV1Error::MaxCllAlreadySet);
        }
        Ok(())
    }

    fn set_max_fall(&self, req: SetMaxFall, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self
            .max_fall
            .replace(Some(F64(req.max_fall as f64)))
            .is_some()
        {
            return Err(WpImageDescriptionCreatorParamsV1Error::MaxFallAlreadySet);
        }
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
    #[error("{} is not a supported named primary", .0)]
    UnsupportedPrimaries(u32),
    #[error("set_tf_power is not supported")]
    SetTfPowerNotSupported,
    #[error("{} is not a supported named transfer function", .0)]
    UnsupportedTf(u32),
    #[error("The transfer function has already been set")]
    TfAlreadySet,
    #[error("The primaries have already been set")]
    PrimariesAlreadySet,
    #[error("The luminances have already been set")]
    LuminancesAlreadySet,
    #[error("The minimum luminance is too low")]
    MinLuminanceTooLow,
    #[error("The transfer function was not set")]
    TfNotSet,
    #[error("The primaries were not set")]
    PrimariesNotSet,
    #[error("The mastering display primaries have already been set")]
    MasteringPrimariesAlreadySet,
    #[error("The mastering display luminances have already been set")]
    MasteringLuminancesAlreadySet,
    #[error("The minimum mastering luminance is too low")]
    MinMasteringLuminanceTooLow,
    #[error("The max CLL has already been set")]
    MaxCllAlreadySet,
    #[error("The max FALL has already been set")]
    MaxFallAlreadySet,
}
efrom!(WpImageDescriptionCreatorParamsV1Error, ClientError);
