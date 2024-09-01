use {
    crate::{gfx_api::GfxContext, utils::oserror::OsError, video::Modifier},
    ahash::AHashMap,
    byteorder::{NativeEndian, WriteBytesExt},
    std::{io::Write, rc::Rc},
    thiserror::Error,
    uapi::{c, OwnedFd},
};

linear_ids!(DrmFeedbackIds, DrmFeedbackId);

#[derive(Debug)]
pub struct DrmFeedbackShared {
    pub fd: Rc<OwnedFd>,
    pub size: usize,
    pub main_device: c::dev_t,
    pub indices: AHashMap<(u32, Modifier), u16>,
}

#[derive(Debug)]
pub struct DrmFeedback {
    pub id: DrmFeedbackId,
    pub shared: Rc<DrmFeedbackShared>,
    pub tranches: Vec<DrmFeedbackTranche>,
}

#[derive(Clone, Debug)]
pub struct DrmFeedbackTranche {
    pub device: c::dev_t,
    pub indices: Vec<u16>,
    pub scanout: bool,
}

impl DrmFeedback {
    pub fn new(
        ids: &DrmFeedbackIds,
        render_ctx: &dyn GfxContext,
    ) -> Result<Self, DrmFeedbackError> {
        let drm = match render_ctx.allocator().drm() {
            Some(drm) => drm.raw(),
            _ => return Err(DrmFeedbackError::NoDrmDevice),
        };
        let main_device = uapi::fstat(drm).map_err(OsError::from)?.st_rdev;
        let (data, index_map) = create_fd_data(render_ctx);
        let mut memfd =
            uapi::memfd_create("drm_feedback", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING).unwrap();
        memfd.write_all(&data).unwrap();
        uapi::lseek(memfd.raw(), 0, c::SEEK_SET).unwrap();
        uapi::fcntl_add_seals(
            memfd.raw(),
            c::F_SEAL_SEAL | c::F_SEAL_GROW | c::F_SEAL_SHRINK | c::F_SEAL_WRITE,
        )
        .unwrap();
        Ok(Self {
            id: ids.next(),
            tranches: vec![DrmFeedbackTranche {
                device: main_device,
                indices: (0..index_map.len()).map(|v| v as u16).collect(),
                scanout: false,
            }],
            shared: Rc::new(DrmFeedbackShared {
                fd: Rc::new(memfd),
                size: data.len(),
                main_device,
                indices: index_map,
            }),
        })
    }

    pub fn for_scanout(
        &self,
        ids: &DrmFeedbackIds,
        devnum: c::dev_t,
        formats: &[(u32, Modifier)],
    ) -> Result<Option<Self>, DrmFeedbackError> {
        let mut tranches = vec![];
        {
            let mut indices = vec![];
            for (format, modifier) in formats {
                if let Some(idx) = self.shared.indices.get(&(*format, *modifier)) {
                    indices.push(*idx);
                }
            }
            if indices.len() > 0 {
                tranches.push(DrmFeedbackTranche {
                    device: devnum,
                    indices,
                    scanout: true,
                });
            } else {
                return Ok(None);
            }
        }
        tranches.extend(self.tranches.iter().cloned());
        Ok(Some(Self {
            id: ids.next(),
            shared: self.shared.clone(),
            tranches,
        }))
    }
}

fn create_fd_data(ctx: &dyn GfxContext) -> (Vec<u8>, AHashMap<(u32, Modifier), u16>) {
    let mut vec = vec![];
    let mut map = AHashMap::new();
    let mut pos = 0;
    for (format, info) in &*ctx.formats() {
        for modifier in &info.read_modifiers {
            vec.write_u32::<NativeEndian>(*format).unwrap();
            vec.write_u32::<NativeEndian>(0).unwrap();
            vec.write_u64::<NativeEndian>(*modifier).unwrap();
            map.insert((*format, *modifier), pos);
            pos += 1;
        }
    }
    (vec, map)
}

#[derive(Debug, Error)]
pub enum DrmFeedbackError {
    #[error("Could not stat drm device")]
    Stat(#[from] OsError),
    #[error("Graphics API does not have a DRM device")]
    NoDrmDevice,
}
