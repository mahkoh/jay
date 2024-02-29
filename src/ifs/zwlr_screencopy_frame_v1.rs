use {
    crate::{
        client::{Client, ClientError},
        format::XRGB8888,
        ifs::{
            wl_buffer::{WlBuffer, WlBufferError, WlBufferStorage},
            wl_output::WlOutputGlobal,
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        utils::{
            buffd::{MsgParser, MsgParserError},
            linkedlist::LinkedNode,
        },
        wire::{zwlr_screencopy_frame_v1::*, WlBufferId, ZwlrScreencopyFrameV1Id},
    },
    std::{cell::Cell, ops::Deref, rc::Rc},
    thiserror::Error,
};

#[allow(dead_code)]
pub const FLAGS_Y_INVERT: u32 = 1;

pub struct ZwlrScreencopyFrameV1 {
    pub id: ZwlrScreencopyFrameV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub output: Rc<WlOutputGlobal>,
    pub rect: Rect,
    pub overlay_cursor: bool,
    pub used: Cell<bool>,
    pub with_damage: Cell<bool>,
    pub output_link: Cell<Option<LinkedNode<Rc<Self>>>>,
    pub buffer: Cell<Option<Rc<WlBuffer>>>,
    pub version: u32,
}

impl ZwlrScreencopyFrameV1 {
    pub fn send_ready(&self, tv_sec: u64, tv_nsec: u32) {
        self.client.event(Ready {
            self_id: self.id,
            tv_sec_hi: (tv_sec >> 32) as u32,
            tv_sec_lo: tv_sec as u32,
            tv_nsec,
        });
    }

    pub fn send_failed(&self) {
        self.client.event(Failed { self_id: self.id });
    }

    pub fn send_damage(&self) {
        let pos = self.output.pos.get();
        self.client.event(Damage {
            self_id: self.id,
            x: 0,
            y: 0,
            width: pos.width() as _,
            height: pos.height() as _,
        });
    }

    pub fn send_buffer(&self) {
        self.client.event(Buffer {
            self_id: self.id,
            format: XRGB8888.wl_id.unwrap(),
            width: self.rect.width() as _,
            height: self.rect.height() as _,
            stride: self.rect.width() as u32 * 4, // TODO
        });
    }

    pub fn send_linux_dmabuf(&self) {
        self.client.event(LinuxDmabuf {
            self_id: self.id,
            format: XRGB8888.drm,
            width: self.rect.width() as _,
            height: self.rect.height() as _,
        });
    }

    pub fn send_buffer_done(&self) {
        self.client.event(BufferDone { self_id: self.id })
    }

    #[allow(dead_code)]
    pub fn send_flags(&self, flags: u32) {
        self.client.event(Flags {
            self_id: self.id,
            flags,
        })
    }

    fn do_copy(
        &self,
        buffer_id: WlBufferId,
        with_damage: bool,
    ) -> Result<(), ZwlrScreencopyFrameV1Error> {
        if self.used.replace(true) {
            return Err(ZwlrScreencopyFrameV1Error::AlreadyUsed);
        }
        let link = match self.output_link.take() {
            Some(l) => l,
            _ => {
                self.send_failed();
                return Ok(());
            }
        };
        let buffer = self.client.lookup(buffer_id)?;
        if (buffer.rect.width(), buffer.rect.height()) != (self.rect.width(), self.rect.height()) {
            return Err(ZwlrScreencopyFrameV1Error::InvalidBufferSize);
        }
        if buffer.format != XRGB8888 {
            return Err(ZwlrScreencopyFrameV1Error::InvalidBufferFormat);
        }
        buffer.update_framebuffer()?;
        if let Some(WlBufferStorage::Shm { stride, .. }) = buffer.storage.borrow_mut().deref() {
            if *stride != self.rect.width() * 4 {
                return Err(ZwlrScreencopyFrameV1Error::InvalidBufferStride);
            }
        }
        self.buffer.set(Some(buffer));
        if !with_damage {
            self.output.connector.connector.damage();
        }
        self.with_damage.set(with_damage);
        self.output.pending_captures.add_last_existing(&link);
        self.output_link.set(Some(link));
        Ok(())
    }

    fn copy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrScreencopyFrameV1Error> {
        let req: Copy = self.client.parse(self, parser)?;
        self.do_copy(req.buffer, false)
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrScreencopyFrameV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        self.output_link.take();
        Ok(())
    }

    fn copy_with_damage(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwlrScreencopyFrameV1Error> {
        let req: CopyWithDamage = self.client.parse(self, parser)?;
        self.do_copy(req.buffer, true)
    }
}

object_base! {
    self = ZwlrScreencopyFrameV1;

    COPY => copy,
    DESTROY => destroy,
    COPY_WITH_DAMAGE => copy_with_damage if self.version >= 2,
}

simple_add_obj!(ZwlrScreencopyFrameV1);

impl Object for ZwlrScreencopyFrameV1 {
    fn break_loops(&self) {
        self.output_link.take();
    }
}

#[derive(Debug, Error)]
pub enum ZwlrScreencopyFrameV1Error {
    #[error("This frame has already been used")]
    AlreadyUsed,
    #[error("The buffer has an invalid size for the frame")]
    InvalidBufferSize,
    #[error("The buffer has an invalid stride for the frame")]
    InvalidBufferStride,
    #[error("The buffer has an invalid format")]
    InvalidBufferFormat,
    #[error(transparent)]
    WlBufferError(Box<WlBufferError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    MsgParserError(Box<MsgParserError>),
}
efrom!(ZwlrScreencopyFrameV1Error, WlBufferError);
efrom!(ZwlrScreencopyFrameV1Error, ClientError);
efrom!(ZwlrScreencopyFrameV1Error, MsgParserError);
