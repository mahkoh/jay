use {
    crate::{
        client::ClientError,
        video::{
            dma::{DmaBuf, DmaBufPlane},
            INVALID_MODIFIER,
        },
        ifs::{wl_buffer::WlBuffer, zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1},
        leaks::Tracker,
        object::Object,
        render::RenderError,
        utils::{
            buffd::{MsgParser, MsgParserError},
            errorfmt::ErrorFmt,
        },
        wire::{zwp_linux_buffer_params_v1::*, WlBufferId, ZwpLinuxBufferParamsV1Id},
    },
    ahash::AHashMap,
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
};

#[allow(dead_code)]
const Y_INVERT: u32 = 1;
#[allow(dead_code)]
const INTERLACED: u32 = 2;
#[allow(dead_code)]
const BOTTOM_FIRST: u32 = 4;

const MAX_PLANE: u32 = 3;

pub struct ZwpLinuxBufferParamsV1 {
    pub id: ZwpLinuxBufferParamsV1Id,
    pub parent: Rc<ZwpLinuxDmabufV1>,
    planes: RefCell<AHashMap<u32, Add>>,
    used: Cell<bool>,
    pub tracker: Tracker<Self>,
}

impl ZwpLinuxBufferParamsV1 {
    pub fn new(id: ZwpLinuxBufferParamsV1Id, parent: &Rc<ZwpLinuxDmabufV1>) -> Self {
        Self {
            id,
            parent: parent.clone(),
            planes: RefCell::new(Default::default()),
            used: Cell::new(false),
            tracker: Default::default(),
        }
    }

    fn send_created(&self, buffer_id: WlBufferId) {
        self.parent.client.event(Created {
            self_id: self.id,
            buffer: buffer_id,
        })
    }

    fn send_failed(&self) {
        self.parent.client.event(Failed { self_id: self.id })
    }

    fn destroy(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.parent.client.parse(&**self, parser)?;
        self.parent.client.remove_obj(&**self)?;
        Ok(())
    }

    fn add(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), AddError> {
        let req: Add = self.parent.client.parse(&**self, parser)?;
        let modifier = ((req.modifier_hi as u64) << 32) | req.modifier_lo as u64;
        if modifier != INVALID_MODIFIER {
            return Err(AddError::InvalidModifier(modifier));
        }
        let plane = req.plane_idx;
        if plane > MAX_PLANE {
            return Err(AddError::MaxPlane);
        }
        if self.planes.borrow_mut().insert(plane, req).is_some() {
            return Err(AddError::AlreadySet(plane));
        }
        Ok(())
    }

    fn do_create(
        self: &Rc<Self>,
        buffer_id: Option<WlBufferId>,
        width: i32,
        height: i32,
        format: u32,
        _flags: u32,
    ) -> Result<WlBufferId, DoCreateError> {
        let ctx = match self.parent.client.state.render_ctx.get() {
            Some(ctx) => ctx,
            None => return Err(DoCreateError::NoRenderContext),
        };
        let formats = ctx.formats();
        let format = match formats.get(&format) {
            Some(f) => *f,
            None => return Err(DoCreateError::InvalidFormat(format)),
        };
        let mut dmabuf = DmaBuf {
            width,
            height,
            format,
            modifier: INVALID_MODIFIER,
            planes: vec![],
        };
        let mut planes: Vec<_> = self.planes.borrow_mut().drain().map(|v| v.1).collect();
        planes.sort_by_key(|a| a.plane_idx);
        for (i, p) in planes.into_iter().enumerate() {
            if p.plane_idx as usize != i {
                return Err(DoCreateError::MissingPlane(i));
            }
            dmabuf.planes.push(DmaBufPlane {
                offset: p.offset,
                stride: p.stride,
                fd: p.fd,
            });
        }
        let img = ctx.dmabuf_img(&dmabuf)?;
        let (is_client_id, buffer_id) = match buffer_id {
            Some(i) => (true, i),
            None => (false, self.parent.client.new_id()?),
        };
        let buffer = Rc::new(WlBuffer::new_dmabuf(
            buffer_id,
            &self.parent.client,
            format,
            &img,
        ));
        track!(self.parent.client, buffer);
        if is_client_id {
            self.parent.client.add_client_obj(&buffer)?;
        } else {
            self.parent.client.add_server_obj(&buffer);
        }
        Ok(buffer_id)
    }

    fn create(self: &Rc<Self>, parser: MsgParser) -> Result<(), CreateError> {
        let req: Create = self.parent.client.parse(&**self, parser)?;
        if self.used.replace(true) {
            return Err(CreateError::AlreadyUsed);
        }
        match self.do_create(None, req.width, req.height, req.format, req.flags) {
            Ok(id) => {
                self.send_created(id);
            }
            Err(e) => {
                log::debug!("Could not create a dmabuf buffer: {}", ErrorFmt(e));
                self.send_failed();
            }
        }
        Ok(())
    }

    fn create_immed(self: &Rc<Self>, parser: MsgParser) -> Result<(), CreateImmedError> {
        let req: CreateImmed = self.parent.client.parse(&**self, parser)?;
        if self.used.replace(true) {
            return Err(CreateImmedError::AlreadyUsed);
        }
        self.do_create(
            Some(req.buffer_id),
            req.width,
            req.height,
            req.format,
            req.flags,
        )?;
        Ok(())
    }
}

object_base! {
    ZwpLinuxBufferParamsV1, ZwpLinuxBufferParamsV1Error;

    DESTROY => destroy,
    ADD => add,
    CREATE => create,
    CREATE_IMMED => create_immed,
}

impl Object for ZwpLinuxBufferParamsV1 {
    fn num_requests(&self) -> u32 {
        CREATE_IMMED + 1
    }
}

simple_add_obj!(ZwpLinuxBufferParamsV1);

#[derive(Debug, Error)]
pub enum ZwpLinuxBufferParamsV1Error {
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `add` request")]
    AddError(#[from] AddError),
    #[error("Could not process a `create` request")]
    Create(#[from] CreateError),
    #[error("Could not process a `create_immed` request")]
    CreateImmed(#[from] CreateImmedError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpLinuxBufferParamsV1Error, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ClientError);
efrom!(DestroyError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum AddError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("A buffer can contain at most 4 planes")]
    MaxPlane,
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The modifier {0} is not supported")]
    InvalidModifier(u64),
    #[error("The plane {0} was already set")]
    AlreadySet(u32),
}
efrom!(AddError, ClientError);
efrom!(AddError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum DoCreateError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The compositor has no render context attached")]
    NoRenderContext,
    #[error("The format {0} is not supported")]
    InvalidFormat(u32),
    #[error("Plane {0} was not set")]
    MissingPlane(usize),
    #[error("Could not import the buffer")]
    ImportError(#[from] RenderError),
}
efrom!(DoCreateError, ClientError);

#[derive(Debug, Error)]
pub enum CreateError {
    #[error("The params object has already been used")]
    AlreadyUsed,
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreateError, ClientError, ClientError);
efrom!(CreateError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreateImmedError {
    #[error("The params object has already been used")]
    AlreadyUsed,
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    DoCreateError(#[from] DoCreateError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreateImmedError, ClientError, ClientError);
efrom!(CreateImmedError, ParseError, MsgParserError);
