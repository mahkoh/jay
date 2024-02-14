use {
    crate::{
        client::ClientError,
        gfx_api::GfxError,
        ifs::{wl_buffer::WlBuffer, zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1},
        leaks::Tracker,
        object::Object,
        utils::{
            buffd::{MsgParser, MsgParserError},
            errorfmt::ErrorFmt,
        },
        video::dmabuf::{DmaBuf, DmaBufPlane, PlaneVec, MAX_PLANES},
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

const MAX_PLANE: u32 = MAX_PLANES as u32 - 1;

pub struct ZwpLinuxBufferParamsV1 {
    pub id: ZwpLinuxBufferParamsV1Id,
    pub parent: Rc<ZwpLinuxDmabufV1>,
    planes: RefCell<AHashMap<u32, Add>>,
    used: Cell<bool>,
    modifier: Cell<Option<u64>>,
    pub tracker: Tracker<Self>,
}

impl ZwpLinuxBufferParamsV1 {
    pub fn new(id: ZwpLinuxBufferParamsV1Id, parent: &Rc<ZwpLinuxDmabufV1>) -> Self {
        Self {
            id,
            parent: parent.clone(),
            planes: RefCell::new(Default::default()),
            used: Cell::new(false),
            modifier: Cell::new(None),
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

    fn destroy(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwpLinuxBufferParamsV1Error> {
        let _req: Destroy = self.parent.client.parse(&**self, parser)?;
        self.parent.client.remove_obj(&**self)?;
        Ok(())
    }

    fn add(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), ZwpLinuxBufferParamsV1Error> {
        let req: Add = self.parent.client.parse(&**self, parser)?;
        let modifier = ((req.modifier_hi as u64) << 32) | req.modifier_lo as u64;
        match self.modifier.get() {
            Some(m) if m != modifier => {
                return Err(ZwpLinuxBufferParamsV1Error::MixedModifiers(modifier, m))
            }
            _ => self.modifier.set(Some(modifier)),
        }
        let plane = req.plane_idx;
        if plane > MAX_PLANE {
            return Err(ZwpLinuxBufferParamsV1Error::MaxPlane);
        }
        if self.planes.borrow_mut().insert(plane, req).is_some() {
            return Err(ZwpLinuxBufferParamsV1Error::AlreadySet(plane));
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
    ) -> Result<WlBufferId, ZwpLinuxBufferParamsV1Error> {
        let ctx = match self.parent.client.state.render_ctx.get() {
            Some(ctx) => ctx,
            None => return Err(ZwpLinuxBufferParamsV1Error::NoRenderContext),
        };
        let formats = ctx.formats();
        let format = match formats.get(&format) {
            Some(f) => f,
            None => return Err(ZwpLinuxBufferParamsV1Error::InvalidFormat(format)),
        };
        let modifier = match self.modifier.get() {
            Some(m) => m,
            _ => return Err(ZwpLinuxBufferParamsV1Error::NoPlanes),
        };
        if !format.read_modifiers.contains(&modifier) {
            return Err(ZwpLinuxBufferParamsV1Error::InvalidModifier(modifier));
        }
        let mut dmabuf = DmaBuf {
            width,
            height,
            format: format.format,
            modifier,
            planes: PlaneVec::new(),
        };
        let mut planes: Vec<_> = self.planes.borrow_mut().drain().map(|v| v.1).collect();
        planes.sort_by_key(|a| a.plane_idx);
        for (i, p) in planes.into_iter().enumerate() {
            if p.plane_idx as usize != i {
                return Err(ZwpLinuxBufferParamsV1Error::MissingPlane(i));
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
            format.format,
            dmabuf,
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

    fn create(self: &Rc<Self>, parser: MsgParser) -> Result<(), ZwpLinuxBufferParamsV1Error> {
        let req: Create = self.parent.client.parse(&**self, parser)?;
        if self.used.replace(true) {
            return Err(ZwpLinuxBufferParamsV1Error::AlreadyUsed);
        }
        match self.do_create(None, req.width, req.height, req.format, req.flags) {
            Ok(id) => {
                self.send_created(id);
            }
            Err(e) => {
                log::warn!("Could not create a dmabuf buffer: {}", ErrorFmt(e));
                self.send_failed();
            }
        }
        Ok(())
    }

    fn create_immed(self: &Rc<Self>, parser: MsgParser) -> Result<(), ZwpLinuxBufferParamsV1Error> {
        let req: CreateImmed = self.parent.client.parse(&**self, parser)?;
        if self.used.replace(true) {
            return Err(ZwpLinuxBufferParamsV1Error::AlreadyUsed);
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
    self = ZwpLinuxBufferParamsV1;

    DESTROY => destroy,
    ADD => add,
    CREATE => create,
    CREATE_IMMED => create_immed if self.parent.version >= 2,
}

impl Object for ZwpLinuxBufferParamsV1 {}

simple_add_obj!(ZwpLinuxBufferParamsV1);

#[derive(Debug, Error)]
pub enum ZwpLinuxBufferParamsV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The params object has already been used")]
    AlreadyUsed,
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("A buffer can contain at most 4 planes")]
    MaxPlane,
    #[error("Tried to add a plane with modifier {0} that differs from a previous modifier {1}")]
    MixedModifiers(u64, u64),
    #[error("The plane {0} was already set")]
    AlreadySet(u32),
    #[error("The compositor has no render context attached")]
    NoRenderContext,
    #[error("The format {0} is not supported")]
    InvalidFormat(u32),
    #[error("No planes were added")]
    NoPlanes,
    #[error("The modifier {0} is not supported")]
    InvalidModifier(u64),
    #[error("Plane {0} was not set")]
    MissingPlane(usize),
    #[error("Could not import the buffer")]
    ImportError(#[from] GfxError),
}
efrom!(ZwpLinuxBufferParamsV1Error, ClientError);
efrom!(ZwpLinuxBufferParamsV1Error, MsgParserError);
