use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::color_management::{
            consts::{
                FEATURE_PARAMETRIC, PRIMARIES_SRGB, RENDER_INTENT_PERCEPTUAL,
                TRANSFER_FUNCTION_SRGB,
            },
            wp_color_management_output_v1::WpColorManagementOutputV1,
            wp_color_management_surface_feedback_v1::WpColorManagementSurfaceFeedbackV1,
            wp_color_management_surface_v1::WpColorManagementSurfaceV1,
            wp_image_description_creator_params_v1::WpImageDescriptionCreatorParamsV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        state::State,
        wire::{
            WpColorManagerV1Id,
            wp_color_manager_v1::{SupportedIntent, *},
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

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
        self.send_supported_feature(FEATURE_PARAMETRIC);
        self.send_supported_tf_named(TRANSFER_FUNCTION_SRGB);
        self.send_supported_primaries_named(PRIMARIES_SRGB);
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
        let _ = self.client.lookup(req.output)?;
        let obj = Rc::new(WpColorManagementOutputV1 {
            id: req.id,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn get_surface(&self, req: GetSurface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let _ = self.client.lookup(req.surface)?;
        let obj = Rc::new(WpColorManagementSurfaceV1 {
            id: req.id,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn get_surface_feedback(
        &self,
        req: GetSurfaceFeedback,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let _ = self.client.lookup(req.surface)?;
        let obj = Rc::new(WpColorManagementSurfaceFeedbackV1 {
            id: req.id,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
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
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn create_windows_scrgb(
        &self,
        _req: CreateWindowsScrgb,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Err(WpColorManagerV1Error::CreateWindowsScrgbNotSupported)
    }
}

global_base!(
    WpColorManagerV1Global,
    WpColorManagerV1,
    WpColorManagerV1Error
);

impl Global for WpColorManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn exposed(&self, state: &State) -> bool {
        state.color_management_enabled.get()
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
    #[error("create_windows_scrgb is not supported")]
    CreateWindowsScrgbNotSupported,
}
efrom!(WpColorManagerV1Error, ClientError);
