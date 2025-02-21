use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        wire::{JayXwaylandId, jay_xwayland::*},
    },
    jay_config::xwayland::XScalingMode,
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayXwayland {
    pub id: JayXwaylandId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayXwayland {
    pub fn send_scaling_mode(&self) {
        let xw = &self.client.state.xwayland;
        self.client.event(ScalingMode {
            self_id: self.id,
            mode: match xw.use_wire_scale.get() {
                false => XScalingMode::DEFAULT.0,
                true => XScalingMode::DOWNSCALED.0,
            },
        });
    }

    pub fn send_implied_scale(&self) {
        let xw = &self.client.state.xwayland;
        if let Some(scale) = xw.wire_scale.get() {
            self.client.event(ImpliedScale {
                self_id: self.id,
                scale,
            });
        }
    }
}

impl JayXwaylandRequestHandler for JayXwayland {
    type Error = JayXwaylandError;

    fn get_scaling(&self, _req: GetScaling, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.send_scaling_mode();
        self.send_implied_scale();
        Ok(())
    }

    fn set_scaling_mode(&self, req: SetScalingMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let use_wire_scale = match XScalingMode(req.mode) {
            XScalingMode::DEFAULT => false,
            XScalingMode::DOWNSCALED => true,
            _ => return Err(JayXwaylandError::UnknownMode(req.mode)),
        };
        self.client
            .state
            .xwayland
            .use_wire_scale
            .set(use_wire_scale);
        self.client.state.update_xwayland_wire_scale();
        Ok(())
    }
}

object_base! {
    self = JayXwayland;
    version = self.version;
}

impl Object for JayXwayland {}

simple_add_obj!(JayXwayland);

#[derive(Debug, Error)]
pub enum JayXwaylandError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Unknown scaling mode {}", .0)]
    UnknownMode(u32),
}
efrom!(JayXwaylandError, ClientError);
