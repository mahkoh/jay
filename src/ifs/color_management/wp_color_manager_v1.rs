use crate::client::Client;
use crate::client::ClientError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::color_management::ABSOLUTE_NO_ADAPTATION_SINCE;
use crate::ifs::color_management::COMPOUND_POWER_2_4_SINCE;
use crate::ifs::color_management::FEATURE_EXTENDED_TARGET_VOLUME;
use crate::ifs::color_management::FEATURE_SET_MASTERING_DISPLAY_PRIMARIES;
use crate::ifs::color_management::FEATURE_SET_TF_POWER;
use crate::ifs::color_management::FEATURE_WINDOWS_BT2100;
use crate::ifs::color_management::RENDER_INTENT_ABSOLUTE_NO_ADAPTATION;
use crate::ifs::color_management::RENDER_INTENT_RELATIVE;
use crate::ifs::color_management::RENDER_INTENT_RELATIVE_BPC;
use crate::ifs::color_management::SRGB_DEPRECATED_SINCE;
use crate::ifs::color_management::TRANSFER_FUNCTION_COMPOUND_POWER_2_4;
use crate::ifs::color_management::WINDOWS_BT2100_SINCE;
use crate::ifs::color_management::consts::FEATURE_PARAMETRIC;
use crate::ifs::color_management::consts::FEATURE_SET_LUMINANCES;
use crate::ifs::color_management::consts::FEATURE_SET_PRIMARIES;
use crate::ifs::color_management::consts::FEATURE_WINDOWS_SCRGB;
use crate::ifs::color_management::consts::PRIMARIES_ADOBE_RGB;
use crate::ifs::color_management::consts::PRIMARIES_BT2020;
use crate::ifs::color_management::consts::PRIMARIES_CIE1931_XYZ;
use crate::ifs::color_management::consts::PRIMARIES_DCI_P3;
use crate::ifs::color_management::consts::PRIMARIES_DISPLAY_P3;
use crate::ifs::color_management::consts::PRIMARIES_GENERIC_FILM;
use crate::ifs::color_management::consts::PRIMARIES_NTSC;
use crate::ifs::color_management::consts::PRIMARIES_PAL;
use crate::ifs::color_management::consts::PRIMARIES_PAL_M;
use crate::ifs::color_management::consts::PRIMARIES_SRGB;
use crate::ifs::color_management::consts::RENDER_INTENT_PERCEPTUAL;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_BT1886;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_EXT_LINEAR;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_EXT_SRGB;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_GAMMA22;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_GAMMA28;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_LOG_100;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_LOG_316;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_SRGB;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_ST240;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_ST428;
use crate::ifs::color_management::consts::TRANSFER_FUNCTION_ST2084_PQ;
use crate::ifs::color_management::wp_color_management_output_v1::WpColorManagementOutputV1;
use crate::ifs::color_management::wp_color_management_surface_feedback_v1::WpColorManagementSurfaceFeedbackV1;
use crate::ifs::color_management::wp_image_description_creator_params_v1::WpImageDescriptionCreatorParamsV1;
use crate::ifs::color_management::wp_image_description_v1::WpImageDescriptionV1;
use crate::ifs::wl_surface::wp_color_management_surface_v1::WpColorManagementSurfaceV1;
use crate::ifs::wl_surface::wp_color_management_surface_v1::WpColorManagementSurfaceV1Error;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::state::State;
use crate::wire::WpColorManagerV1Id;
use crate::wire::wp_color_manager_v1::SupportedIntent;
use crate::wire::wp_color_manager_v1::*;
use std::rc::Rc;
use thiserror::Error;

pub struct WpColorManagerV1Global {
    pub name: GlobalName,
}

impl WpColorManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpColorManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpColorManagerV1Error> {
        let obj = Rc::new(WpColorManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        obj.send_capabilities();
        Ok(())
    }
}

pub struct WpColorManagerV1 {
    pub id: WpColorManagerV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl WpColorManagerV1 {
    fn send_capabilities(&self) {
        self.send_supported_intent(RENDER_INTENT_PERCEPTUAL);
        self.send_supported_intent(RENDER_INTENT_RELATIVE);
        self.send_supported_intent(RENDER_INTENT_RELATIVE_BPC);
        if self.version >= ABSOLUTE_NO_ADAPTATION_SINCE {
            self.send_supported_intent(RENDER_INTENT_ABSOLUTE_NO_ADAPTATION);
        }
        self.send_supported_feature(FEATURE_PARAMETRIC);
        self.send_supported_feature(FEATURE_SET_PRIMARIES);
        self.send_supported_feature(FEATURE_SET_LUMINANCES);
        self.send_supported_feature(FEATURE_SET_MASTERING_DISPLAY_PRIMARIES);
        self.send_supported_feature(FEATURE_SET_TF_POWER);
        self.send_supported_feature(FEATURE_EXTENDED_TARGET_VOLUME);
        self.send_supported_feature(FEATURE_WINDOWS_SCRGB);
        if self.version >= WINDOWS_BT2100_SINCE {
            self.send_supported_feature(FEATURE_WINDOWS_BT2100);
        }
        self.send_supported_tf_named(TRANSFER_FUNCTION_BT1886);
        self.send_supported_tf_named(TRANSFER_FUNCTION_GAMMA22);
        self.send_supported_tf_named(TRANSFER_FUNCTION_GAMMA28);
        self.send_supported_tf_named(TRANSFER_FUNCTION_ST240);
        self.send_supported_tf_named(TRANSFER_FUNCTION_EXT_LINEAR);
        self.send_supported_tf_named(TRANSFER_FUNCTION_LOG_100);
        self.send_supported_tf_named(TRANSFER_FUNCTION_LOG_316);
        if self.version < SRGB_DEPRECATED_SINCE {
            self.send_supported_tf_named(TRANSFER_FUNCTION_SRGB);
            self.send_supported_tf_named(TRANSFER_FUNCTION_EXT_SRGB);
        }
        self.send_supported_tf_named(TRANSFER_FUNCTION_ST2084_PQ);
        self.send_supported_tf_named(TRANSFER_FUNCTION_ST428);
        if self.version >= COMPOUND_POWER_2_4_SINCE {
            self.send_supported_tf_named(TRANSFER_FUNCTION_COMPOUND_POWER_2_4);
        }
        self.send_supported_primaries_named(PRIMARIES_SRGB);
        self.send_supported_primaries_named(PRIMARIES_PAL_M);
        self.send_supported_primaries_named(PRIMARIES_PAL);
        self.send_supported_primaries_named(PRIMARIES_NTSC);
        self.send_supported_primaries_named(PRIMARIES_GENERIC_FILM);
        self.send_supported_primaries_named(PRIMARIES_BT2020);
        self.send_supported_primaries_named(PRIMARIES_CIE1931_XYZ);
        self.send_supported_primaries_named(PRIMARIES_DCI_P3);
        self.send_supported_primaries_named(PRIMARIES_DISPLAY_P3);
        self.send_supported_primaries_named(PRIMARIES_ADOBE_RGB);
        self.send_done();
    }

    fn send_supported_intent(&self, render_intent: u32) {
        self.client.event(SupportedIntent {
            self_id: self.id,
            render_intent,
        });
    }

    fn send_supported_feature(&self, feature: u32) {
        self.client.event(SupportedFeature {
            self_id: self.id,
            feature,
        });
    }

    fn send_supported_tf_named(&self, tf: u32) {
        self.client.event(SupportedTfNamed {
            self_id: self.id,
            tf,
        });
    }

    fn send_supported_primaries_named(&self, primaries: u32) {
        self.client.event(SupportedPrimariesNamed {
            self_id: self.id,
            primaries,
        });
    }

    fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }
}

impl WpColorManagerV1RequestHandler for WpColorManagerV1 {
    type Error = WpColorManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_output(&self, req: GetOutput, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = self.client.lookup(req.output)?;
        let obj = Rc::new(WpColorManagementOutputV1 {
            id: req.id,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            output: output.global.clone(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        if let Some(node) = output.global.node() {
            node.color_description_listeners
                .set((self.client.id, req.id), obj);
        }
        Ok(())
    }

    fn get_surface(&self, req: GetSurface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let obj = Rc::new(WpColorManagementSurfaceV1 {
            id: req.id,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            surface: surface.clone(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.install()?;
        Ok(())
    }

    fn get_surface_feedback(
        &self,
        req: GetSurfaceFeedback,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let obj = Rc::new(WpColorManagementSurfaceFeedbackV1 {
            id: req.id,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            surface: surface.clone(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        surface.add_color_management_feedback(&obj);
        Ok(())
    }

    fn create_icc_creator(
        &self,
        _req: CreateIccCreator,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Err(WpColorManagerV1Error::CreateIccCreatorNotSupported)
    }

    fn create_parametric_creator(
        &self,
        req: CreateParametricCreator,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let obj = Rc::new(WpImageDescriptionCreatorParamsV1 {
            id: req.obj,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            tf: Default::default(),
            primaries: Default::default(),
            luminance: Default::default(),
            mastering_primaries: Default::default(),
            mastering_luminance: Default::default(),
            max_cll: Default::default(),
            max_fall: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn create_windows_scrgb(
        &self,
        req: CreateWindowsScrgb,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let obj = Rc::new(WpImageDescriptionV1 {
            id: req.image_description,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            description: Some(self.client.state.color_manager.windows_scrgb().clone()),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_ready();
        Ok(())
    }

    fn get_image_description(
        &self,
        req: GetImageDescription,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let desc = self.client.lookup(req.reference)?;
        let obj = Rc::new(WpImageDescriptionV1 {
            id: req.image_description,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            description: Some(desc.description.clone()),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_ready();
        Ok(())
    }

    fn create_windows_bt2100(
        &self,
        req: CreateWindowsBt2100,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let obj = Rc::new(WpImageDescriptionV1 {
            id: req.image_description,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            description: Some(self.client.state.color_manager.windows_bt2100().clone()),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.send_ready();
        Ok(())
    }
}

global_base!(
    WpColorManagerV1Global,
    WpColorManagerV1,
    WpColorManagerV1Error
);

impl Global for WpColorManagerV1Global {
    fn version(&self) -> u32 {
        3
    }

    fn exposed(&self, state: &State) -> bool {
        state.color_management_available()
    }
}

simple_add_global!(WpColorManagerV1Global);

object_base! {
    self = WpColorManagerV1;
    version = self.version;
}

impl Object for WpColorManagerV1 {}

simple_add_obj!(WpColorManagerV1);

#[derive(Debug, Error)]
pub enum WpColorManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("create_icc_creator is not supported")]
    CreateIccCreatorNotSupported,
    #[error(transparent)]
    Surface(#[from] WpColorManagementSurfaceV1Error),
}
efrom!(WpColorManagerV1Error, ClientError);
