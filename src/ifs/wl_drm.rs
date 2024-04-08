use {
    crate::{
        client::{Client, ClientError},
        gfx_api::GfxError,
        globals::{Global, GlobalName},
        ifs::wl_buffer::WlBuffer,
        leaks::Tracker,
        object::{Object, Version},
        utils::buffd::{MsgParser, MsgParserError},
        video::{
            dmabuf::{DmaBuf, DmaBufPlane, PlaneVec},
            INVALID_MODIFIER,
        },
        wire::{wl_drm::*, WlDrmId},
    },
    bstr::ByteSlice,
    std::{ffi::CString, rc::Rc},
    thiserror::Error,
};

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
    version: Version,
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

    fn authenticate(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), WlDrmError> {
        let _req: Authenticate = self.client.parse(&**self, parser)?;
        self.send_authenticated();
        Ok(())
    }

    fn create_buffer(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), WlDrmError> {
        let _req: CreateBuffer = self.client.parse(&**self, parser)?;
        Err(WlDrmError::Unsupported)
    }

    fn create_planar_buffer(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), WlDrmError> {
        let _req: CreatePlanarBuffer = self.client.parse(&**self, parser)?;
        Err(WlDrmError::Unsupported)
    }

    fn create_prime_buffer(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), WlDrmError> {
        let req: CreatePrimeBuffer = self.client.parse(&**self, parser)?;
        let ctx = match self.client.state.render_ctx.get() {
            Some(ctx) => ctx,
            None => return Err(WlDrmError::NoRenderContext),
        };
        let formats = ctx.formats();
        let format = match formats.get(&req.format) {
            Some(f) => f.format,
            None => return Err(WlDrmError::InvalidFormat(req.format)),
        };
        let mut dmabuf = DmaBuf {
            id: self.client.state.dma_buf_ids.next(),
            width: req.width,
            height: req.height,
            format,
            modifier: INVALID_MODIFIER,
            planes: PlaneVec::new(),
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
        let buffer = Rc::new(WlBuffer::new_dmabuf(
            req.id,
            &self.client,
            format,
            dmabuf,
            &img,
        ));
        track!(self.client, buffer);
        self.client.add_client_obj(&buffer)?;
        Ok(())
    }
}

object_base! {
    self = WlDrm;

    AUTHENTICATE => authenticate,
    CREATE_BUFFER => create_buffer,
    CREATE_PLANAR_BUFFER => create_planar_buffer,
    CREATE_PRIME_BUFFER => create_prime_buffer if self.version >= 2,
}

impl Object for WlDrm {}

simple_add_obj!(WlDrm);

#[derive(Debug, Error)]
pub enum WlDrmError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("This api is not supported")]
    Unsupported,
    #[error("The compositor has no render context attached")]
    NoRenderContext,
    #[error("The format {0} is not supported")]
    InvalidFormat(u32),
    #[error("Could not import the buffer")]
    ImportError(#[from] GfxError),
}
efrom!(WlDrmError, ClientError);
efrom!(WlDrmError, MsgParserError);
