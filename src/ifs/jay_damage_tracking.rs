use {
    crate::{
        client::{CAP_JAY_COMPOSITOR, Client, ClientCaps, ClientError},
        cmm::cmm_eotf::Eotf,
        globals::{Global, GlobalName},
        leaks::Tracker,
        object::{Object, Version},
        theme::Color,
        wire::{
            JayCompositorId,
            jay_damage_tracking::{
                Destroy, JayDamageTrackingRequestHandler, SetVisualizerColor, SetVisualizerDecay,
                SetVisualizerEnabled,
            },
        },
    },
    std::{rc::Rc, time::Duration},
    thiserror::Error,
};

pub struct JayDamageTrackingGlobal {
    name: GlobalName,
}

impl JayDamageTrackingGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: JayCompositorId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), JayDamageTrackingError> {
        let obj = Rc::new(JayDamageTracking {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    JayDamageTrackingGlobal,
    JayDamageTracking,
    JayDamageTrackingError
);

impl Global for JayDamageTrackingGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_JAY_COMPOSITOR
    }
}

simple_add_global!(JayDamageTrackingGlobal);

pub struct JayDamageTracking {
    id: JayCompositorId,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl JayDamageTrackingRequestHandler for JayDamageTracking {
    type Error = JayDamageTrackingError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_visualizer_enabled(
        &self,
        req: SetVisualizerEnabled,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let state = &self.client.state;
        state.damage_visualizer.set_enabled(state, req.enabled != 0);
        Ok(())
    }

    fn set_visualizer_color(
        &self,
        req: SetVisualizerColor,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let color = Color::new(Eotf::Gamma22, req.r, req.g, req.b) * req.a;
        self.client.state.damage_visualizer.set_color(color);
        Ok(())
    }

    fn set_visualizer_decay(
        &self,
        req: SetVisualizerDecay,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.client
            .state
            .damage_visualizer
            .set_decay(Duration::from_millis(req.millis));
        Ok(())
    }
}

object_base! {
    self = JayDamageTracking;
    version = self.version;
}

impl Object for JayDamageTracking {}

simple_add_obj!(JayDamageTracking);

#[derive(Debug, Error)]
pub enum JayDamageTrackingError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayDamageTrackingError, ClientError);
