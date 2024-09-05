use {
    crate::{
        format::formats,
        gfx_api::{GfxFormat, GfxWriteModifier},
        ifs::jay_render_ctx::FORMATS_SINCE,
        object::Version,
        utils::clonecell::CloneCell,
        video::Modifier,
        wire::{jay_render_ctx::*, JayRenderCtxId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    ahash::AHashMap,
    std::{cell::RefCell, convert::Infallible, rc::Rc},
    uapi::OwnedFd,
};

pub struct UsrJayRenderCtx {
    pub id: JayRenderCtxId,
    pub con: Rc<UsrCon>,
    pub owner: CloneCell<Option<Rc<dyn UsrJayRenderCtxOwner>>>,
    pub version: Version,
    pub formats: RefCell<AHashMap<u32, GfxFormat>>,
}

pub trait UsrJayRenderCtxOwner {
    fn no_device(&self) {}
    fn device(&self, fd: Rc<OwnedFd>, server_formats: Option<AHashMap<u32, GfxFormat>>) {
        let _ = fd;
        let _ = server_formats;
    }
}

impl UsrJayRenderCtx {
    fn add_write_modifier(&self, format: u32, modifier: Modifier, needs_render_usage: bool) {
        if let Some(format) = self.formats.borrow_mut().get_mut(&format) {
            format
                .write_modifiers
                .insert(modifier, GfxWriteModifier { needs_render_usage });
        }
    }
}

impl JayRenderCtxEventHandler for UsrJayRenderCtx {
    type Error = Infallible;

    fn no_device(&self, _ev: NoDevice, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.formats.take();
        if let Some(owner) = self.owner.get() {
            owner.no_device();
        }
        Ok(())
    }

    fn device(&self, ev: Device, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let formats = self.formats.take();
        let formats = (self.version >= FORMATS_SINCE).then_some(formats);
        if let Some(owner) = self.owner.get() {
            owner.device(ev.fd, formats);
        }
        Ok(())
    }

    fn read_modifier(&self, ev: ReadModifier, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(format) = self.formats.borrow_mut().get_mut(&ev.format) {
            format.read_modifiers.insert(ev.modifier);
        }
        Ok(())
    }

    fn write_modifier(&self, ev: WriteModifier, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.add_write_modifier(ev.format, ev.modifier, true);
        Ok(())
    }

    fn format(&self, ev: Format, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(format) = formats().get(&ev.format) {
            self.formats.borrow_mut().insert(
                ev.format,
                GfxFormat {
                    format,
                    read_modifiers: Default::default(),
                    write_modifiers: Default::default(),
                },
            );
        }
        Ok(())
    }

    fn write_modifier2(&self, ev: WriteModifier2, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.add_write_modifier(ev.format, ev.modifier, ev.needs_render_usage != 0);
        Ok(())
    }
}

usr_object_base! {
    self = UsrJayRenderCtx = JayRenderCtx;
    version = self.version;
}

impl UsrObject for UsrJayRenderCtx {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }

    fn break_loops(&self) {
        self.owner.take();
    }
}
