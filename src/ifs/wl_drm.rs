use crate::client::{Client, ClientError};
use crate::drm::dma::{DmaBuf, DmaBufPlane};
use crate::drm::INVALID_MODIFIER;
use crate::globals::{Global, GlobalName};
use crate::ifs::wl_buffer::WlBuffer;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::render::RenderError;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_drm::*;
use crate::wire::WlDrmId;
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
        version: u32,
    ) -> Result<(), WlDrmError> {
        let obj = Rc::new(WlDrm {
            id,
            client: client.clone(),
            _version: version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        if let Some(rc) = client.state.render_ctx.get() {
            obj.send_device(&rc.render_node());
            obj.send_capabilities(PRIME);
        }
        Ok(())
    }
}

global_base!(WlDrmGlobal, WlDrm, WlDrmError);

impl Global for WlDrmGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        2
    }
}

simple_add_global!(WlDrmGlobal);

pub struct WlDrm {
    id: WlDrmId,
    pub client: Rc<Client>,
    _version: u32,
    tracker: Tracker<Self>,
}

impl WlDrm {
    fn send_device(self: &Rc<Self>, device: &Rc<CString>) {
        self.client.event(Device {
            self_id: self.id,
            name: device.as_bytes().as_bstr(),
        })
    }

    fn send_authenticated(self: &Rc<Self>) {
        self.client.event(Authenticated { self_id: self.id })
    }

    fn send_capabilities(self: &Rc<Self>, value: u32) {
        self.client.event(Capabilities {
            self_id: self.id,
            value,
        })
    }

    fn authenticate(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), AuthenticateError> {
        let _req: Authenticate = self.client.parse(&**self, parser)?;
        self.send_authenticated();
        Ok(())
    }

    fn create_buffer(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), CreateBufferError> {
        let _req: CreateBuffer = self.client.parse(&**self, parser)?;
        Err(CreateBufferError::Unsupported)
    }

    fn create_planar_buffer(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), CreatePlanarBufferError> {
        let _req: CreatePlanarBuffer = self.client.parse(&**self, parser)?;
        Err(CreatePlanarBufferError::Unsupported)
    }

    fn create_prime_buffer(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), CreatePrimeBufferError> {
        let req: CreatePrimeBuffer = self.client.parse(&**self, parser)?;
        let ctx = match self.client.state.render_ctx.get() {
            Some(ctx) => ctx,
            None => return Err(CreatePrimeBufferError::NoRenderContext),
        };
        let formats = ctx.formats();
        let format = match formats.get(&req.format) {
            Some(f) => *f,
            None => return Err(CreatePrimeBufferError::InvalidFormat(req.format)),
        };
        let mut dmabuf = DmaBuf {
            width: req.width,
            height: req.height,
            format,
            modifier: INVALID_MODIFIER,
            planes: vec![],
        };
        if req.stride0 > 0 {
            dmabuf.planes.push(DmaBufPlane {
                offset: req.offset0 as _,
                stride: req.stride0 as _,
                fd: req.name.clone(),
            });
            if req.stride1 > 0 {
                dmabuf.planes.push(DmaBufPlane {
                    offset: req.offset1 as _,
                    stride: req.stride1 as _,
                    fd: req.name.clone(),
                });
                if req.stride2 > 0 {
                    dmabuf.planes.push(DmaBufPlane {
                        offset: req.offset2 as _,
                        stride: req.stride2 as _,
                        fd: req.name.clone(),
                    });
                }
            }
        }
        let img = ctx.dmabuf_img(&dmabuf)?;
        let buffer = Rc::new(WlBuffer::new_dmabuf(req.id, &self.client, format, &img));
        track!(self.client, buffer);
        self.client.add_client_obj(&buffer)?;
        Ok(())
    }
}

object_base! {
    WlDrm, WlDrmError;

    AUTHENTICATE => authenticate,
    CREATE_BUFFER => create_buffer,
    CREATE_PLANAR_BUFFER => create_planar_buffer,
    CREATE_PRIME_BUFFER => create_prime_buffer,
}

impl Object for WlDrm {
    fn num_requests(&self) -> u32 {
        CREATE_PRIME_BUFFER + 1
    }
}

simple_add_obj!(WlDrm);

#[derive(Debug, Error)]
pub enum WlDrmError {
    #[error("Could not process a `authenticate` request")]
    AuthenticateError(#[from] AuthenticateError),
    #[error("Could not process a `create_buffer` request")]
    CreateBufferError(#[from] CreateBufferError),
    #[error("Could not process a `create_planar_buffer` request")]
    CreatePlanarBufferError(#[from] CreatePlanarBufferError),
    #[error("Could not process a `create_prime_buffer` request")]
    CreatePrimeBufferError(#[from] CreatePrimeBufferError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlDrmError, ClientError);

#[derive(Debug, Error)]
pub enum AuthenticateError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(AuthenticateError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreateBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("This api is not supported")]
    Unsupported,
}
efrom!(CreateBufferError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreatePlanarBufferError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("This api is not supported")]
    Unsupported,
}
efrom!(CreatePlanarBufferError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreatePrimeBufferError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("The compositor has no render context attached")]
    NoRenderContext,
    #[error("The format {0} is not supported")]
    InvalidFormat(u32),
    #[error("Could not import the buffer")]
    ImportError(#[from] RenderError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreatePrimeBufferError, MsgParserError);
efrom!(CreatePrimeBufferError, ClientError);