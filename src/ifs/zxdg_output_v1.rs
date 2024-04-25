use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_output::{WlOutput, SEND_DONE_SINCE},
        leaks::Tracker,
        object::{Object, Version},
        wire::{zxdg_output_v1::*, ZxdgOutputV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub const NAME_SINCE: Version = Version(2);
#[allow(dead_code)]
pub const DESCRIPTION_SINCE: Version = Version(2);
pub const NO_DONE_SINCE: Version = Version(3);

pub struct ZxdgOutputV1 {
    pub id: ZxdgOutputV1Id,
    pub version: Version,
    pub client: Rc<Client>,
    pub output: Rc<WlOutput>,
    pub tracker: Tracker<Self>,
}

impl ZxdgOutputV1 {
    pub fn send_logical_position(&self, x: i32, y: i32) {
        self.client.event(LogicalPosition {
            self_id: self.id,
            x,
            y,
        });
    }

    pub fn send_logical_size(&self, width: i32, height: i32) {
        self.client.event(LogicalSize {
            self_id: self.id,
            width,
            height,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_name(&self, name: &str) {
        self.client.event(Name {
            self_id: self.id,
            name,
        });
    }

    #[allow(dead_code)]
    pub fn send_description(&self, description: &str) {
        self.client.event(Description {
            self_id: self.id,
            description,
        });
    }

    pub fn send_updates(&self) {
        let Some(global) = self.output.global.get() else {
            return;
        };
        let pos = global.position();
        self.send_logical_position(pos.x1(), pos.y1());
        self.send_logical_size(pos.width(), pos.height());
        if self.version >= NAME_SINCE {
            self.send_name(&global.connector.name);
        }
        if self.version >= NO_DONE_SINCE {
            if self.output.version >= SEND_DONE_SINCE {
                self.output.send_done();
            }
        } else {
            self.send_done();
        }
    }
}

impl ZxdgOutputV1RequestHandler for ZxdgOutputV1 {
    type Error = ZxdgOutputV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.output.xdg_outputs.remove(&self.id);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZxdgOutputV1;
    version = self.version;
}

impl Object for ZxdgOutputV1 {}

simple_add_obj!(ZxdgOutputV1);

#[derive(Debug, Error)]
pub enum ZxdgOutputV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZxdgOutputV1Error, ClientError);
