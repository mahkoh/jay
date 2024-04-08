use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::{x_surface::XSurface, WlSurfaceError},
        leaks::Tracker,
        object::{Object, Version},
        utils::cell_ext::CellExt,
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
    pub version: Version,
}

impl XwaylandSurfaceV1RequestHandler for XwaylandSurfaceV1 {
    type Error = XwaylandSurfaceV1Error;

    fn set_serial(&self, req: SetSerial, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.x.surface.xwayland_serial.is_some() {
            return Err(XwaylandSurfaceV1Error::SerialAlreadySet);
        }
        let serial = req.serial_lo as u64 | ((req.serial_hi as u64) << 32);
        if self.client.last_xwayland_serial.get() >= serial {
            return Err(XwaylandSurfaceV1Error::NonMonotonicSerial);
        }
        self.client.last_xwayland_serial.set(serial);
        self.x.surface.pending.borrow_mut().xwayland_serial = Some(serial);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.x.xwayland_surface.set(None);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = XwaylandSurfaceV1;
    version = self.version;
}

impl Object for XwaylandSurfaceV1 {
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
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(XwaylandSurfaceV1Error, ClientError);
