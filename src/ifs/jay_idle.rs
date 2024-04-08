use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{jay_idle::*, JayIdleId},
    },
    std::{rc::Rc, time::Duration},
    thiserror::Error,
};

pub struct JayIdle {
    pub id: JayIdleId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl JayIdle {
    fn send_interval(&self) {
        let to = self.client.state.idle.timeout.get();
        self.client.event(Interval {
            self_id: self.id,
            interval: to.as_secs(),
        });
    }

    fn send_inhibitor(&self, surface: &ZwpIdleInhibitorV1) {
        let surface = &surface.surface;
        self.client.event(Inhibitor {
            self_id: self.id,
            surface: surface.id,
            client_id: surface.client.id.raw(),
            pid: surface.client.pid_info.pid as _,
            comm: &surface.client.pid_info.comm,
        });
    }
}

impl JayIdleRequestHandler for JayIdle {
    type Error = JayIdleError;

    fn get_status(&self, _req: GetStatus, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.send_interval();
        {
            let inhibitors = self.client.state.idle.inhibitors.lock();
            for inhibitor in inhibitors.values() {
                self.send_inhibitor(inhibitor);
            }
        }
        Ok(())
    }

    fn set_interval(&self, req: SetInterval, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let interval = Duration::from_secs(req.interval);
        self.client.state.idle.set_timeout(interval);
        Ok(())
    }
}

object_base! {
    self = JayIdle;
    version = Version(1);
}

impl Object for JayIdle {}

simple_add_obj!(JayIdle);

#[derive(Debug, Error)]
pub enum JayIdleError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayIdleError, ClientError);
