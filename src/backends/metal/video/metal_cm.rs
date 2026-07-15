use crate::backend::BackendGammaLut;
use crate::backend::BackendGammaLutId;
use crate::backends::metal::video::CollectedProperties;
use crate::backends::metal::video::MetalCrtc;
use crate::backends::metal::video::MetalDrmVendor;
use crate::backends::metal::video::MetalPlane;
use crate::backends::metal::video::metal_cm::metal_cm_crtc::CrtcColorPipelines;
use crate::backends::metal::video::metal_cm::metal_cm_crtc::metal_cm_crtc_matcher::CrtcMatcherCache;
use crate::backends::metal::video::metal_cm::metal_cm_crtc::metal_cm_crtc_matcher::GammaLutKey;
use crate::backends::metal::video::metal_cm::metal_cm_matcher::MatcherCache;
use crate::backends::metal::video::metal_cm::metal_cm_matcher::compute_programming;
use crate::backends::metal::video::metal_cm::metal_cm_plane::PlaneColorPipelines;
use crate::backends::metal::video::metal_cm::metal_cm_plane::metal_cm_plane_matcher::Lut1dKey;
use crate::backends::metal::video::metal_cm::metal_cm_plane::metal_cm_plane_matcher::Lut3dKey;
use crate::backends::metal::video::metal_cm::metal_cm_plane::metal_cm_plane_matcher::MatrixKey;
use crate::backends::metal::video::metal_cm::metal_cm_plane::metal_cm_plane_matcher::PlaneMatcherCache;
use crate::cmm::cmm_description::ColorDescription;
use crate::cmm::cmm_description::ColorDescriptionId;
use crate::cmm::cmm_eotf::Eotf;
use crate::cmm::cmm_luminance::Luminance;
use crate::cmm::cmm_render_intent::RenderIntent;
use crate::env::JAY_MCM_AMD_ALLOW_CURSOR;
use crate::utils::clonecell::CloneCell;
use crate::utils::obj_and_id::ObjWithId;
use crate::utils::object_registry::CachedObjectRegistry;
use crate::utils::object_registry::CachedRegisteredObject;
use crate::utils::object_registry::ObjectRegistry;
use crate::utils::object_registry::UncachedObjectRegistry;
use crate::utils::object_registry::UncachedRegisteredObject;
use crate::video::drm::Change;
use crate::video::drm::DrmBlob;
use crate::video::drm::DrmColorop;
use crate::video::drm::DrmCrtc;
use crate::video::drm::DrmMaster;
use crate::video::drm::DrmPlane;
use crate::video::drm::DrmPropertyValue;
use crate::video::drm::Logging;
use crate::video::drm::PrepareDrmObjectProperties;
use crate::video::drm::PropBlob;
use jay_proc::PrepareDrmObjectProperties;
use jay_proc::jay_hash;
use std::cell::Cell;
use std::rc::Rc;

mod metal_cm_crtc;
mod metal_cm_lut;
mod metal_cm_matcher;
mod metal_cm_paths;
mod metal_cm_plane;

// AMD: 4096
// Nvidia: 1024
// Intel: between 32 and 128, i.e., useless
const MIN_LUT_SIZE: usize = 1024;

pub struct MetalCmDevice {
    shared: Rc<Shared>,
}

struct Shared {
    vendor: MetalDrmVendor,
    programmings: CachedObjectRegistry<ProgrammingKey, Programming>,
    plane_matcher_cache: PlaneMatcherCache,
    crtc_matcher_cache: CrtcMatcherCache,
    blob_registry: UncachedObjectRegistry<BlobRegistryKey, PropBlob>,
    matcher_cache: MatcherCache,
}

#[jay_hash]
#[derive(Copy, Clone, Debug)]
enum BlobRegistryKey {
    GammaLut(GammaLutKey),
    Lut1d(Lut1dKey),
    Matrix(MatrixKey),
    Lut3d(Lut3dKey),
}

type RegisteredBlob = Rc<UncachedRegisteredObject<BlobRegistryKey, PropBlob>>;

pub struct MetalCmConnector {
    shared: Rc<Shared>,
    current: CloneCell<Option<CachedProgramming>>,
    current_ptr: Cell<*const CachedRegisteredObject<ProgrammingKey, Programming>>,
    previous: CloneCell<Option<CachedProgramming>>,
    last_failed: Cell<Option<ProgrammingKey>>,
}

pub struct MetalCmPlane {
    id: DrmPlane,
    pipelines: PlaneColorPipelines,
}

pub struct MetalCmCrtc {
    id: DrmCrtc,
    pipelines: CrtcColorPipelines,
}

type CachedProgramming = Rc<CachedRegisteredObject<ProgrammingKey, Programming>>;

#[jay_hash]
#[derive(Copy, Clone, Debug)]
struct ProgrammingKey {
    plane: DrmPlane,
    crtc: DrmCrtc,
    src_description: ColorDescriptionId,
    dst_description: ColorDescriptionId,
    intent: RenderIntent,
    gamma_lut: Option<BackendGammaLutId>,
    has_cursor_plane: bool,
    use_plane_color_pipelines: bool,
}

#[derive(Debug)]
pub struct MetalCmProgramming {
    programming: CachedProgramming,
}

#[derive(Debug)]
struct Programming {
    failed: bool,
    plane: PlaneProgramming,
    plane_color_ops: Vec<PlaneColorOpProgramming>,
    crtc_color_ops: Vec<CrtcColorOpProgramming>,
}

#[derive(Debug, Default)]
struct PlaneProgramming {
    id: DrmPlane,
    props: PlaneProps,
}

#[derive(PrepareDrmObjectProperties, Debug, Default)]
struct PlaneProps {
    pipeline: Option<DrmPropertyValue<DrmColorop>>,
}

#[derive(Default, Debug)]
struct PlaneColorOpProgramming {
    id: DrmColorop,
    props: PlaneColorOpProps,
    _data_blob: Option<RegisteredBlob>,
}

#[derive(PrepareDrmObjectProperties, Default, Debug)]
struct PlaneColorOpProps {
    bypass: Option<DrmPropertyValue>,
    ty: Option<DrmPropertyValue>,
    interpolation: Option<DrmPropertyValue>,
    data_id: Option<DrmPropertyValue<DrmBlob>>,
    multiplier: Option<DrmPropertyValue>,
}

#[derive(Default, Debug)]
struct CrtcColorOpProgramming {
    id: DrmCrtc,
    props: CrtcColorOpProps,
    _gamma_lut_blob: Option<RegisteredBlob>,
}

#[derive(PrepareDrmObjectProperties, Default, Debug)]
struct CrtcColorOpProps {
    gamma_lut: Option<DrmPropertyValue<DrmBlob>>,
}

impl MetalCmConnector {
    pub fn invalidate(&self) {
        self.shared.programmings.clear();
        self.previous.set(self.current.take());
        self.current_ptr.set(Default::default());
        self.last_failed.set(Default::default());
    }

    pub fn find_programming(
        &self,
        master: &Rc<DrmMaster>,
        plane: &Rc<MetalPlane>,
        crtc: &Rc<MetalCrtc>,
        src: &Rc<ColorDescription>,
        dst: &Rc<ColorDescription>,
        intent: RenderIntent,
        gamma_lut: Option<&Rc<BackendGammaLut>>,
        has_cursor_plane: bool,
        mut use_plane_color_pipelines: bool,
    ) -> Option<MetalCmProgramming> {
        if self.shared.vendor.is_amd && has_cursor_plane && !*JAY_MCM_AMD_ALLOW_CURSOR {
            // https://gitlab.freedesktop.org/drm/amd/-/work_items/5062#note_3569710
            use_plane_color_pipelines = false;
        }
        let key = ProgrammingKey {
            plane: plane.id,
            crtc: crtc.id,
            src_description: src.id,
            dst_description: dst.id,
            intent,
            gamma_lut: gamma_lut.id(),
            has_cursor_plane,
            use_plane_color_pipelines,
        };
        if let Some(programming) = self.current.get()
            && programming.key() == &key
        {
            return Some(MetalCmProgramming { programming });
        }
        if self.last_failed.get() == Some(key) {
            return None;
        }
        if let Some(programming) = self.previous.get()
            && programming.key() == &key
        {
            return Some(MetalCmProgramming { programming });
        }
        if let Some(programming) = self.shared.programmings.get(&key) {
            if programming.failed {
                programming.mark_used();
                self.last_failed.set(Some(key));
                return None;
            }
            return Some(MetalCmProgramming { programming });
        }
        let programming = compute_programming(
            master,
            &self.shared,
            &plane.cm,
            &crtc.cm,
            src,
            dst,
            intent,
            gamma_lut,
            has_cursor_plane,
            use_plane_color_pipelines,
        );
        let programming = self.shared.programmings.insert(key, programming);
        if programming.failed {
            self.last_failed.set(Some(key));
            return None;
        }
        Some(MetalCmProgramming { programming })
    }

    pub fn prepare(
        &self,
        programming: &MetalCmProgramming,
        change: &mut Change,
        logging: Option<&Logging>,
    ) {
        let v = &programming.programming;
        if Rc::as_ptr(v) == self.current_ptr.get() {
            return;
        }
        macro_rules! log_changing {
            ($ty:literal, $id:expr) => {
                if let Some(logging) = logging {
                    log::log!(target: logging.target, logging.level, concat!("changing ", $ty, " {:?}"), $id);
                }
            };
        }
        change.change_object(v.plane.id, |c| {
            log_changing!("plane", v.plane.id);
            v.plane.props.prepare(c, logging);
        });
        for op in &v.plane_color_ops {
            change.change_object(op.id, |c| {
                log_changing!("color op", op.id);
                op.props.prepare(c, logging);
            });
        }
        for op in &v.crtc_color_ops {
            change.change_object(op.id, |c| {
                log_changing!("crtc", op.id);
                op.props.prepare(c, logging);
            });
        }
    }

    pub fn apply(&self, programming: &MetalCmProgramming) {
        let p = &programming.programming;
        p.mark_used();
        if self.current_ptr.get() == Rc::as_ptr(p) {
            return;
        }
        self.current_ptr.set(Rc::as_ptr(p));
        let current = self.current.set(Some(p.clone()));
        self.previous.set(current);
    }

    pub fn current(&self) -> Option<MetalCmProgramming> {
        Some(MetalCmProgramming {
            programming: self.current.get()?,
        })
    }

    pub fn is_different(&self, programming: &MetalCmProgramming) -> bool {
        Rc::as_ptr(&programming.programming) != self.current_ptr.get()
    }
}

impl MetalCmConnector {
    pub fn new(dev: &MetalCmDevice) -> Self {
        Self {
            shared: dev.shared.clone(),
            current: Default::default(),
            current_ptr: Default::default(),
            previous: Default::default(),
            last_failed: Default::default(),
        }
    }
}

impl MetalCmPlane {
    pub(super) fn new(
        master: &Rc<DrmMaster>,
        vendor: &MetalDrmVendor,
        plane: DrmPlane,
        props: &CollectedProperties,
    ) -> Self {
        Self {
            id: plane,
            pipelines: PlaneColorPipelines::new(master, vendor, plane, props),
        }
    }
}

impl MetalCmCrtc {
    pub(super) fn new(crtc: DrmCrtc, props: &CollectedProperties) -> Self {
        Self {
            id: crtc,
            pipelines: CrtcColorPipelines::new(crtc, props),
        }
    }
}

impl MetalCmDevice {
    pub fn new(vendor: &MetalDrmVendor) -> Self {
        Self {
            shared: Rc::new(Shared {
                vendor: *vendor,
                programmings: ObjectRegistry::with_cache(128),
                plane_matcher_cache: Default::default(),
                crtc_matcher_cache: Default::default(),
                blob_registry: Default::default(),
                matcher_cache: Default::default(),
            }),
        }
    }
}

fn dst_lut_out_scale(desc: &Rc<ColorDescription>) -> Option<f64> {
    if desc.eotf == Eotf::St2084Pq {
        Some(Luminance::ST2084_PQ.max.0 / desc.linear.target_luminance.max.0)
    } else {
        None
    }
}
