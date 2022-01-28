use crate::client::DynEventFormatter;
use crate::drm::dma::{DmaBuf, DmaBufPlane};
use crate::drm::INVALID_MODIFIER;
use crate::ifs::wl_buffer::{WlBuffer, WlBufferId};
use crate::ifs::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1Obj;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use crate::ErrorFmt;
use ahash::AHashMap;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
pub use types::*;

mod types;

const DESTROY: u32 = 0;
const ADD: u32 = 1;
const CREATE: u32 = 2;
const CREATE_IMMED: u32 = 3;

const CREATED: u32 = 0;
const FAILED: u32 = 1;

#[allow(dead_code)]
const Y_INVERT: u32 = 1;
#[allow(dead_code)]
const INTERLACED: u32 = 2;
#[allow(dead_code)]
const BOTTOM_FIRST: u32 = 4;

id!(ZwpLinuxBufferParamsV1Id);

const MAX_PLANE: u32 = 3;

pub struct ZwpLinuxBufferParamsV1 {
    id: ZwpLinuxBufferParamsV1Id,
    parent: Rc<ZwpLinuxDmabufV1Obj>,
    planes: RefCell<AHashMap<u32, Add>>,
    used: Cell<bool>,
}

impl ZwpLinuxBufferParamsV1 {
    pub fn new(id: ZwpLinuxBufferParamsV1Id, parent: &Rc<ZwpLinuxDmabufV1Obj>) -> Self {
        Self {
            id,
            parent: parent.clone(),
            planes: RefCell::new(Default::default()),
            used: Cell::new(false),
        }
    }

    fn created(self: &Rc<Self>, buffer_id: WlBufferId) -> DynEventFormatter {
        Box::new(Created {
            obj: self.clone(),
            buffer: buffer_id,
        })
    }

    fn failed(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Failed { obj: self.clone() })
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
        if is_client_id {
            self.parent.client.add_client_obj(&buffer)?;
        } else {
            self.parent.client.add_server_obj(&buffer);
        }
        Ok(buffer_id)
    }

    fn create(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), CreateError> {
        let req: Create = self.parent.client.parse(&**self, parser)?;
        if self.used.replace(true) {
            return Err(CreateError::AlreadyUsed);
        }
        match self.do_create(None, req.width, req.height, req.format, req.flags) {
            Ok(id) => {
                self.parent.client.event(self.created(id));
            }
            Err(e) => {
                log::debug!("Could not create a dmabuf buffer: {}", ErrorFmt(e));
                self.parent.client.event(self.failed());
            }
        }
        Ok(())
    }

    fn create_immed(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), CreateImmedError> {
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

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwpLinuxBufferParamsV1Error> {
        match request {
            DESTROY => self.destroy(parser)?,
            ADD => self.add(parser)?,
            CREATE => self.create(parser)?,
            CREATE_IMMED => self.create_immed(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(ZwpLinuxBufferParamsV1);

impl Object for ZwpLinuxBufferParamsV1 {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::ZwpLinuxBufferParamsV1
    }

    fn num_requests(&self) -> u32 {
        CREATE_IMMED + 1
    }
}
