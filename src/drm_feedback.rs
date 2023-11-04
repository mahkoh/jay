use {
    crate::{gfx_api::GfxContext, utils::oserror::OsError},
    byteorder::{NativeEndian, WriteBytesExt},
    std::{io::Write, rc::Rc},
    thiserror::Error,
    uapi::{c, OwnedFd},
};

pub struct DrmFeedback {
    pub fd: Rc<OwnedFd>,
    pub size: usize,
    pub indices: Vec<u16>,
    pub main_device: c::dev_t,
}

impl DrmFeedback {
    pub fn new(ctx: &dyn GfxContext) -> Result<Self, DrmFeedbackError> {
        let dev_t = uapi::fstat(ctx.gbm().drm.raw())
            .map_err(OsError::from)?
            .st_rdev;
        let data = create_fd_data(ctx);
        let mut memfd =
            uapi::memfd_create("drm_feedback", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING).unwrap();
        memfd.write_all(&data).unwrap();
        uapi::lseek(memfd.raw(), 0, c::SEEK_SET).unwrap();
        uapi::fcntl_add_seals(
            memfd.raw(),
            c::F_SEAL_SEAL | c::F_SEAL_GROW | c::F_SEAL_SHRINK | c::F_SEAL_WRITE,
        )
        .unwrap();
        let num_indices = data.len() / 16;
        let indices = (0..num_indices).map(|v| v as u16).collect();
        Ok(Self {
            fd: Rc::new(memfd),
            size: data.len(),
            indices,
            main_device: dev_t,
        })
    }
}

fn create_fd_data(ctx: &dyn GfxContext) -> Vec<u8> {
    let mut vec = vec![];
    for (format, info) in &*ctx.formats() {
        for modifier in &info.modifiers {
            vec.write_u32::<NativeEndian>(*format).unwrap();
            vec.write_u32::<NativeEndian>(0).unwrap();
            vec.write_u64::<NativeEndian>(*modifier).unwrap();
        }
    }
    vec
}

#[derive(Debug, Error)]
pub enum DrmFeedbackError {
    #[error("Could not stat drm device")]
    Stat(#[from] OsError),
}
