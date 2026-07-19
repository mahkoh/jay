use crate::backend::ConnectorId;
use crate::client::ClientId;
use crate::format::formats;
use crate::ifs::zwp_linux_dmabuf_feedback_v1::FB_SAMPLING;
use crate::ifs::zwp_linux_dmabuf_feedback_v1::FB_SCANOUT;
use crate::ifs::zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1;
use crate::object::Version;
use crate::state::State;
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::bhash::BHashSet;
use crate::utils::clonecell::CloneCell;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::hash_map_ext::HashMapExt;
use crate::utils::hash_map_ext::HashSetExt;
use crate::utils::oserror::OsError;
use crate::utils::oserror::OsErrorExt2;
use crate::video::Modifier;
use crate::wire::ZwpLinuxDmabufFeedbackV1Id;
use byteorder::NativeEndian;
use byteorder::WriteBytesExt;
use indexmap::IndexSet;
use isnt::std_1::primitive::IsntSliceExt;
use std::io::Write;
use std::io::{self};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;

#[derive(Debug, Error)]
enum DmaBufFeedbackError {
    #[error("Table is too large: 0x{0:x}")]
    TooLarge(usize),
    #[error("Could not create a memfd")]
    CreateMemfd(#[source] OsError),
    #[error("Could not write to memfd")]
    WriteMemfd(#[source] io::Error),
    #[error("Could not seek memfd")]
    SeekMemfd(#[source] OsError),
    #[error("Could not seal memfd")]
    SealMemfd(#[source] OsError),
    #[error("Format index {0:x?} does not fit into u16")]
    IndexOutOfBounds(Pair),
}

linear_ids!(DmaBufFeedbackIds, DmaBufFeedbackId, u64);

#[derive(Default)]
pub struct DmaBufFeedbackState {
    pub fb: CloneCell<Option<Rc<DmaBufFeedback>>>,
    pub default: CopyHashMap<(ClientId, ZwpLinuxDmabufFeedbackV1Id), Rc<ZwpLinuxDmabufFeedbackV1>>,
    ids: DmaBufFeedbackIds,
    changed: AsyncEvent,
}

pub struct DmaBufFeedback {
    id: DmaBufFeedbackId,
    memfd: Rc<OwnedFd>,
    size: u32,
    main_dev: c::dev_t,
    tranches: Vec<Tranche>,
}

type Pair = (u32, Modifier);

#[derive(PartialEq)]
struct Tranche {
    dev_t: c::dev_t,
    pairs: Vec<Pair>,
    indices: Vec<u16>,
    ty: TrancheType,
    prime: bool,
}

#[derive(Clone, PartialEq)]
enum TrancheType {
    Scanout { connector: ConnectorId },
    Sampling,
}

pub async fn handle_dmabuf_feedback_changes(state: Rc<State>) {
    loop {
        state.dmabuf_feedback.changed.triggered().await;
        state.update_dmabuf_feedback();
    }
}

impl DmaBufFeedbackState {
    pub fn update(&self) {
        self.changed.trigger();
    }
}

impl State {
    fn update_dmabuf_feedback(&self) {
        let fb = self
            .update_dmabuf_feedback_()
            .inspect_err(|e| {
                log::error!("Could not update dmabuf feedback: {}", ErrorFmt(e));
            })
            .ok()
            .flatten();
        let Some(fb) = fb else {
            return;
        };
        for zfb in self.dmabuf_feedback.default.lock().values() {
            fb.send(zfb, None);
        }
        for client in self.clients.clients.borrow().values() {
            for surface in client.data.objects.surfaces.lock().values() {
                surface.send_feedback(&fb);
            }
        }
    }

    fn update_dmabuf_feedback_(&self) -> Result<Option<Rc<DmaBufFeedback>>, DmaBufFeedbackError> {
        let Some(ctx) = self.render_ctx.get() else {
            self.dmabuf_feedback.fb.set(None);
            return Ok(None);
        };
        let Some(ctx_dev) = ctx.drm_device_id().and_then(|id| self.drm_devs.get(&id)) else {
            self.dmabuf_feedback.fb.set(None);
            return Ok(None);
        };
        let main_dev = ctx_dev.dev_t;
        let ctx_formats = ctx.formats();
        let mut copy_src_formats = BHashSet::default();
        if !self.no_client_prime {
            for dev in self.drm_devs.lock().values() {
                let Some(copy_dev) = &dev.copy_device else {
                    continue;
                };
                for &format in formats().values() {
                    for modifier in copy_dev.src_support(format) {
                        copy_src_formats.insert((dev.id, format.drm, modifier.modifier));
                    }
                }
            }
        }
        let prime_formats = self.render_ctx_prime_modifiers.get(&None);
        let mut tranches = vec![];
        let mut connectors: Vec<_> = self.connectors.lock().values().cloned().collect();
        connectors.sort_by_key(|c| c.id);
        for connector in &connectors {
            let Some(dev) = &connector.drm_dev else {
                continue;
            };
            let prime = dev.dev_t != main_dev;
            let Some(scanout_formats) = connector.connector.scanout_formats() else {
                continue;
            };
            let mut pairs = vec![];
            if prime {
                if self.no_client_prime {
                    continue;
                }
                let Some(prime_formats) = &prime_formats else {
                    continue;
                };
                for &(format, modifier) in &*scanout_formats {
                    if prime_formats.not_contains_key(&format.drm) {
                        continue;
                    }
                    if copy_src_formats.not_contains(&(dev.id, format.drm, modifier)) {
                        continue;
                    }
                    pairs.push((format.drm, modifier));
                }
            } else {
                for &(format, modifier) in &*scanout_formats {
                    if let Some(ctx_format) = ctx_formats.get(&format.drm)
                        && ctx_format.read_modifiers.contains(&modifier)
                    {
                        pairs.push((format.drm, modifier));
                    }
                }
            }
            if pairs.is_not_empty() {
                tranches.push(Tranche {
                    dev_t: dev.dev_t,
                    pairs,
                    indices: Default::default(),
                    ty: TrancheType::Scanout {
                        connector: connector.id,
                    },
                    prime,
                });
            }
        }
        {
            let mut pairs = vec![];
            for (&fmt, modifiers) in ctx_formats.iter() {
                for &modifier in &modifiers.read_modifiers {
                    pairs.push((fmt, modifier));
                }
            }
            tranches.push(Tranche {
                dev_t: main_dev,
                pairs,
                indices: Default::default(),
                ty: TrancheType::Sampling,
                prime: false,
            });
        }
        if !self.no_client_prime
            && let Some(prime_formats) = &prime_formats
        {
            let mut devs: Vec<_> = self.drm_devs.lock().values().cloned().collect();
            devs.sort_by_key(|d| d.id);
            for dev in &devs {
                if dev.id == ctx_dev.id {
                    continue;
                }
                let Some(copy_dev) = &dev.copy_device else {
                    continue;
                };
                let mut pairs = vec![];
                for &format in formats().values() {
                    if prime_formats.not_contains_key(&format.drm) {
                        continue;
                    }
                    for support in copy_dev.src_support(format) {
                        pairs.push((format.drm, support.modifier));
                    }
                }
                if pairs.is_not_empty() {
                    tranches.push(Tranche {
                        dev_t: dev.dev_t,
                        pairs,
                        indices: Default::default(),
                        ty: TrancheType::Sampling,
                        prime: true,
                    });
                }
            }
        }
        let indices = create_indices(&mut tranches)?;
        if let Some(old) = self.dmabuf_feedback.fb.get()
            && old.tranches == tranches
            && old.main_dev == main_dev
        {
            return Ok(None);
        }
        let (memfd, size) = create_memfd(&indices)?;
        let fb = Rc::new(DmaBufFeedback {
            id: self.dmabuf_feedback.ids.next(),
            memfd,
            size,
            main_dev,
            tranches,
        });
        self.dmabuf_feedback.fb.set(Some(fb.clone()));
        Ok(Some(fb))
    }
}

fn create_indices(tranches: &mut [Tranche]) -> Result<IndexSet<Pair>, DmaBufFeedbackError> {
    let mut indices = IndexSet::new();
    for tranche in tranches {
        tranche.pairs.sort_by_key(|p| p.0);
        for pair @ &(format, modifier) in &tranche.pairs {
            let (index, _) = indices.insert_full((format, modifier));
            let Ok(index) = u16::try_from(index) else {
                return Err(DmaBufFeedbackError::IndexOutOfBounds(*pair));
            };
            tranche.indices.push(index);
        }
    }
    Ok(indices)
}

fn create_memfd(indices: &IndexSet<Pair>) -> Result<(Rc<OwnedFd>, u32), DmaBufFeedbackError> {
    let mut vec = Vec::with_capacity(indices.len() * 16);
    for &(format, modifier) in indices {
        vec.write_u32::<NativeEndian>(format).unwrap();
        vec.write_u32::<NativeEndian>(0).unwrap();
        vec.write_u64::<NativeEndian>(modifier).unwrap();
    }
    let Ok(size) = u32::try_from(vec.len()) else {
        return Err(DmaBufFeedbackError::TooLarge(vec.len()));
    };
    let mut memfd = uapi::memfd_create("dmabuf-feedback", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING)
        .map_os_err(DmaBufFeedbackError::CreateMemfd)?;
    memfd
        .write_all(&vec)
        .map_err(DmaBufFeedbackError::WriteMemfd)?;
    uapi::lseek(memfd.raw(), 0, c::SEEK_SET).map_os_err(DmaBufFeedbackError::SeekMemfd)?;
    uapi::fcntl_add_seals(
        memfd.raw(),
        c::F_SEAL_SEAL | c::F_SEAL_GROW | c::F_SEAL_SHRINK | c::F_SEAL_WRITE,
    )
    .map_os_err(DmaBufFeedbackError::SealMemfd)?;
    Ok((Rc::new(memfd), size))
}

const SAMPLING_SINCE: Version = Version(6);

impl DmaBufFeedback {
    pub fn send(&self, obj: &ZwpLinuxDmabufFeedbackV1, fullscreen: Option<ConnectorId>) {
        let id = Some(self.id);
        if obj.last_format_table.replace(id) != id {
            obj.send_format_table(&self.memfd, self.size);
        }
        if obj.version < SAMPLING_SINCE {
            obj.send_main_device(self.main_dev);
        }
        for tranche in &self.tranches {
            if tranche.prime && obj.version < SAMPLING_SINCE {
                continue;
            }
            let mut flags = match tranche.ty {
                TrancheType::Scanout { connector } => {
                    if Some(connector) != fullscreen {
                        continue;
                    }
                    FB_SCANOUT
                }
                TrancheType::Sampling => 0,
            };
            if obj.version >= SAMPLING_SINCE {
                flags |= FB_SAMPLING;
            }
            obj.send_tranche_target_device(tranche.dev_t);
            obj.send_tranche_formats(&tranche.indices);
            obj.send_tranche_flags(flags);
            obj.send_tranche_done();
        }
        obj.send_done();
    }
}
