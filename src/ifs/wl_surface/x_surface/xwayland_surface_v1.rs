use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::{x_surface::XSurface, WlSurfaceError},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{xwayland_surface_v1::*, XwaylandSurfaceV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct XwaylandSurfaceV1 {
    pub id: XwaylandSurfaceV1Id,
    pub client: Rc<Client>,
    pub x: Rc<XSurface>,
    pub tracker: Tracker<Self>,
}

impl XwaylandSurfaceV1 {
    fn set_serial(&self, parser: MsgParser<'_, '_>) -> Result<(), XwaylandSurfaceV1Error> {
        let req: SetSerial = self.client.parse(self, parser)?;
        if self.x.surface.xwayland_serial.get().is_some() {
            return Err(XwaylandSurfaceV1Error::SerialAlreadySet);
        }
        let serial = req.serial_lo as u64 | ((req.serial_hi as u64) << 32);
        if self.client.last_xwayland_serial.get() >= serial {
            return Err(XwaylandSurfaceV1Error::NonMonotonicSerial);
        }
        self.client.last_xwayland_serial.set(serial);
        self.x.surface.pending.xwayland_serial.set(Some(serial));
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), XwaylandSurfaceV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.x.xwayland_surface.set(None);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    XwaylandSurfaceV1;

    SET_SERIAL => set_serial,
    DESTROY => destroy,
}

impl Object for XwaylandSurfaceV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        self.x.xwayland_surface.set(None);
    }
}

simple_add_obj!(XwaylandSurfaceV1);

#[derive(Debug, Error)]
pub enum XwaylandSurfaceV1Error {
    #[error("The serial for this surface is already set")]
    SerialAlreadySet,
    #[error("The serial is not larger than the previously used serial")]
    NonMonotonicSerial,
    #[error(transparent)]
    WlSurfaceError(#[from] WlSurfaceError),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(XwaylandSurfaceV1Error, MsgParserError);
efrom!(XwaylandSurfaceV1Error, ClientError);
