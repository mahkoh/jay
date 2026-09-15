use crate::client::Client;
use crate::client::LookupError;
use crate::ifs::wl_buffer::SyntheticWlBuffer;
use crate::ifs::wl_buffer::WlBuffer;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::fx_hash::FHashMap;
use crate::wire::XdgToplevelIconV1Id;
use crate::wire::xdg_toplevel_icon_v1::*;
use jay_proc::Object;
use jay_proc::jay_hash;
use std::cell::OnceCell;
use std::rc::Rc;
use thiserror::Error;

#[derive(Object)]
pub struct XdgToplevelIconV1 {
    id: XdgToplevelIconV1Id,
    client: Rc<Client>,
    pub tracker: Tracker<Self>,
    version: Version,
    synthetic_buffers: OnceCell<Rc<FHashMap<BufferKey, SyntheticWlBuffer>>>,
    buffers: CopyHashMap<BufferKey, Rc<WlBuffer>>,
}

#[jay_hash]
#[derive(Copy, Clone, Debug, Eq)]
pub struct BufferKey {
    pub size: i32,
    pub scale: i32,
}

impl XdgToplevelIconV1 {
    pub fn new(id: XdgToplevelIconV1Id, client: &Rc<Client>, version: Version) -> Rc<Self> {
        Rc::new(Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            synthetic_buffers: Default::default(),
            buffers: Default::default(),
        })
    }

    fn check_immutable(&self) -> Result<(), XdgToplevelIconV1Error> {
        if self.synthetic_buffers.get().is_some() {
            return Err(XdgToplevelIconV1Error::Immutable);
        }
        Ok(())
    }

    pub fn buffers(&self) -> &Rc<FHashMap<BufferKey, SyntheticWlBuffer>> {
        self.synthetic_buffers.get_or_init(|| {
            let mut map = FHashMap::default();
            for (k, v) in self.buffers.lock().iter() {
                map.insert(*k, v.clone_synthetic());
            }
            Rc::new(map)
        })
    }
}

impl XdgToplevelIconV1RequestHandler for XdgToplevelIconV1 {
    type Error = XdgToplevelIconV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn set_name(&self, _req: SetName<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.check_immutable()?;
        Ok(())
    }

    fn add_buffer(&self, req: AddBuffer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.check_immutable()?;
        let buffer = self.client.lookup(req.buffer)?;
        if buffer.rect.width() != buffer.rect.height() {
            return Err(XdgToplevelIconV1Error::NotSquare);
        }
        let key = BufferKey {
            size: buffer.rect.width(),
            scale: req.scale,
        };
        self.buffers.set(key, buffer);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum XdgToplevelIconV1Error {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error("Toplevel icon is immutable")]
    Immutable,
    #[error("Buffer is not a square")]
    NotSquare,
}
