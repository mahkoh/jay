use crate::client::Client;
use crate::client::ClientError;
use crate::format::formats;
use crate::gfx_api::GfxError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_buffer::WlBuffer;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::state::State;
use crate::video::INVALID_MODIFIER;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufPlane;
use crate::video::dmabuf::PlaneVec;
use crate::wire::WlDrmId;
use crate::wire::wl_drm::*;
use bstr::ByteSlice;
use std::ffi::CString;
use std::rc::Rc;
use thiserror::Error;

const PRIME: u32 = 1;

pub struct WlDrmGlobal {
    name: GlobalName,
}

impl WlDrmGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlDrmId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WlDrmError> {
        let obj = Rc::new(WlDrm {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        if let Some(rc) = client.state.render_ctx.get() {
            if let Some(rn) = rc.render_node() {
                obj.send_device(&rn);
            }
            obj.send_capabilities(PRIME);
        }
        Ok(())
    }
}

global_base!(WlDrmGlobal, WlDrm, WlDrmError);

impl Global for WlDrmGlobal {
    fn version(&self) -> u32 {
        2
    }

    fn exposed(&self, state: &State) -> bool {
        let Some(ctx) = state.render_ctx.get() else {
            return false;
        };
        ctx.supports_invalid_modifier()
    }
}

simple_add_global!(WlDrmGlobal);

pub struct WlDrm {
    id: WlDrmId,
    pub client: Rc<Client>,
    version: Version,
    tracker: Tracker<Self>,
}

impl WlDrm {
    fn send_device(&self, device: &Rc<CString>) {
        self.client.event(Device {
            self_id: self.id,
            name: device.as_bytes().as_bstr(),
        })
    }

    fn send_authenticated(&self) {
        self.client.event(Authenticated { self_id: self.id })
    }

    fn send_capabilities(&self, value: u32) {
        self.client.event(Capabilities {
            self_id: self.id,
            value,
        })
    }
}

impl WlDrmRequestHandler for WlDrm {
    type Error = WlDrmError;

    fn authenticate(&self, _req: Authenticate, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.send_authenticated();
        Ok(())
    }

    fn create_buffer(&self, _req: CreateBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Err(WlDrmError::Unsupported)
    }

    fn create_planar_buffer(
        &self,
        _req: CreatePlanarBuffer,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Err(WlDrmError::Unsupported)
    }

    fn create_prime_buffer(
        &self,
        req: CreatePrimeBuffer,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let state = &self.client.state;
        let format = match formats().get(&req.format) {
            Some(f) => *f,
            None => return Err(WlDrmError::InvalidFormat(req.format)),
        };
        let mut planes = PlaneVec::new();
        if req.stride0 > 0 {
            planes.push(DmaBufPlane {
                offset: req.offset0 as _,
                stride: req.stride0 as _,
                fd: req.name.clone(),
            });
            if req.stride1 > 0 {
                planes.push(DmaBufPlane {
                    offset: req.offset1 as _,
                    stride: req.stride1 as _,
                    fd: req.name.clone(),
                });
                if req.stride2 > 0 {
                    planes.push(DmaBufPlane {
                        offset: req.offset2 as _,
                        stride: req.stride2 as _,
                        fd: req.name.clone(),
                    });
                }
            }
        }
        let dmabuf = DmaBuf::new(
            &state.dma_buf_ids,
            req.width,
            req.height,
            format,
            INVALID_MODIFIER,
            planes,
        );
        let buffer = WlBuffer::new_dmabuf(req.id, &self.client, format, dmabuf, None);
        track!(self.client, buffer);
        self.client.add_client_obj(&buffer)?;
        Ok(())
    }
}

object_base! {
    self = WlDrm;
    version = self.version;
}

impl Object for WlDrm {}

simple_add_obj!(WlDrm);

#[derive(Debug, Error)]
pub enum WlDrmError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("This api is not supported")]
    Unsupported,
    #[error("The format {0} is not supported")]
    InvalidFormat(u32),
    #[error("Could not import the buffer")]
    ImportError(#[from] GfxError),
}
efrom!(WlDrmError, ClientError);
