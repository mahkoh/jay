use crate::cmm::cmm_description::ColorDescription;
use crate::cmm::cmm_description::ColorDescriptionIds;
use crate::cmm::cmm_description::LinearColorDescription;
use crate::cmm::cmm_description::LinearColorDescriptionId;
use crate::cmm::cmm_description::LinearColorDescriptionIds;
use crate::cmm::cmm_eotf::Eotf;
use crate::cmm::cmm_luminance::Luminance;
use crate::cmm::cmm_luminance::TargetLuminance;
use crate::cmm::cmm_primaries::NamedPrimaries;
use crate::cmm::cmm_primaries::Primaries;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::numcell::NumCell;
use crate::utils::ordered_float::F64;
use jay_proc::jay_hash;
use std::rc::Rc;
use std::rc::Weak;

pub struct ColorManager {
    linear_ids: LinearColorDescriptionIds,
    linear_descriptions: CopyHashMap<LinearDescriptionKey, Weak<LinearColorDescription>>,
    complete_descriptions: CopyHashMap<CompleteDescriptionKey, Weak<ColorDescription>>,
    shared: Rc<Shared>,
    srgb_gamma22: Rc<ColorDescription>,
    srgb_linear: Rc<ColorDescription>,
    windows_scrgb: Rc<ColorDescription>,
    windows_bt2100: Rc<ColorDescription>,
}

#[derive(Debug, Default)]
pub(super) struct Shared {
    pub(super) dead_linear: NumCell<usize>,
    pub(super) dead_complete: NumCell<usize>,
    pub(super) complete_ids: ColorDescriptionIds,
}

#[jay_hash]
#[derive(Copy, Clone, Debug, Eq)]
struct LinearDescriptionKey {
    primaries: Primaries,
    luminance: Luminance,
    target_primaries: Primaries,
    target_luminance: TargetLuminance,
    max_cll: Option<F64>,
    max_fall: Option<F64>,
}

#[jay_hash]
#[derive(Copy, Clone, Debug, Eq)]
struct CompleteDescriptionKey {
    linear: LinearColorDescriptionId,
    named_primaries: Option<NamedPrimaries>,
    eotf: Eotf,
}

impl ColorManager {
    pub fn new() -> Rc<Self> {
        let linear_ids = LinearColorDescriptionIds::default();
        let linear_descriptions = CopyHashMap::default();
        let complete_descriptions = CopyHashMap::default();
        let shared = Rc::new(Shared::default());
        let _ = shared.complete_ids.next();
        let srgb_gamma22 = get_description(
            &shared,
            &linear_descriptions,
            &complete_descriptions,
            &linear_ids,
            Some(NamedPrimaries::Srgb),
            Primaries::SRGB,
            Luminance::SRGB,
            Eotf::Gamma22,
            Primaries::SRGB,
            Luminance::SRGB.to_target(),
            None,
            None,
        );
        let srgb_linear = get_description2(
            &shared,
            &srgb_gamma22.linear,
            &complete_descriptions,
            Some(NamedPrimaries::Srgb),
            Eotf::Linear,
        );
        let windows_scrgb = get_description(
            &shared,
            &linear_descriptions,
            &complete_descriptions,
            &linear_ids,
            Some(NamedPrimaries::Srgb),
            Primaries::SRGB,
            Luminance::WINDOWS_SCRGB,
            Eotf::Linear,
            Primaries::BT2020,
            Luminance::ST2084_PQ.to_target(),
            None,
            None,
        );
        let windows_bt2100 = get_description(
            &shared,
            &linear_descriptions,
            &complete_descriptions,
            &linear_ids,
            Some(NamedPrimaries::Bt2020),
            Primaries::BT2020,
            Luminance::ST2084_PQ,
            Eotf::St2084Pq,
            Primaries::BT2020,
            Luminance::ST2084_PQ.to_target(),
            None,
            None,
        );
        Rc::new(Self {
            linear_ids,
            linear_descriptions,
            complete_descriptions,
            shared,
            srgb_gamma22,
            srgb_linear,
            windows_scrgb,
            windows_bt2100,
        })
    }

    pub fn srgb_gamma22(&self) -> &Rc<ColorDescription> {
        &self.srgb_gamma22
    }

    pub fn srgb_linear(&self) -> &Rc<ColorDescription> {
        &self.srgb_linear
    }

    pub fn windows_scrgb(&self) -> &Rc<ColorDescription> {
        &self.windows_scrgb
    }

    pub fn windows_bt2100(&self) -> &Rc<ColorDescription> {
        &self.windows_bt2100
    }

    pub fn get_description(
        self: &Rc<Self>,
        named_primaries: Option<NamedPrimaries>,
        primaries: Primaries,
        luminance: Luminance,
        eotf: Eotf,
        target_primaries: Primaries,
        target_luminance: TargetLuminance,
        max_cll: Option<F64>,
        max_fall: Option<F64>,
    ) -> Rc<ColorDescription> {
        get_description(
            &self.shared,
            &self.linear_descriptions,
            &self.complete_descriptions,
            &self.linear_ids,
            named_primaries,
            primaries,
            luminance,
            eotf,
            target_primaries,
            target_luminance,
            max_cll,
            max_fall,
        )
    }

    pub fn get_with_tf(
        self: &Rc<Self>,
        cd: &Rc<ColorDescription>,
        eotf: Eotf,
    ) -> Rc<ColorDescription> {
        get_description2(
            &self.shared,
            &cd.linear,
            &self.complete_descriptions,
            cd.named_primaries,
            eotf,
        )
    }
}

fn get_description(
    shared: &Rc<Shared>,
    linear_descriptions: &CopyHashMap<LinearDescriptionKey, Weak<LinearColorDescription>>,
    complete_descriptions: &CopyHashMap<CompleteDescriptionKey, Weak<ColorDescription>>,
    linear_ids: &LinearColorDescriptionIds,
    named_primaries: Option<NamedPrimaries>,
    primaries: Primaries,
    luminance: Luminance,
    eotf: Eotf,
    target_primaries: Primaries,
    target_luminance: TargetLuminance,
    max_cll: Option<F64>,
    max_fall: Option<F64>,
) -> Rc<ColorDescription> {
    macro_rules! gc {
        ($d:ident, $i:expr) => {
            if $d.len() > 16 && $i.get() * 2 > $d.len() {
                $d.lock().retain(|_, d| d.strong_count() > 0);
                $i.set(0);
            }
        };
    }
    gc!(linear_descriptions, &shared.dead_linear);
    gc!(complete_descriptions, &shared.dead_complete);
    let key = LinearDescriptionKey {
        primaries,
        luminance,
        target_primaries,
        target_luminance,
        max_cll,
        max_fall,
    };
    if let Some(d) = linear_descriptions.get(&key) {
        if let Some(d) = d.upgrade() {
            return get_description2(shared, &d, complete_descriptions, named_primaries, eotf);
        }
        shared.dead_linear.fetch_sub(1);
    }
    let (xyz_from_local, local_from_xyz) = primaries.matrices();
    let d = Rc::new(LinearColorDescription {
        id: linear_ids.next(),
        primaries,
        xyz_from_local,
        local_from_xyz,
        luminance,
        target_primaries,
        target_luminance,
        target_contained_in_primary: Default::default(),
        max_cll,
        max_fall,
        shared: shared.clone(),
    });
    linear_descriptions.set(key, Rc::downgrade(&d));
    let key = CompleteDescriptionKey {
        linear: d.id,
        named_primaries,
        eotf,
    };
    let d = Rc::new(ColorDescription {
        id: shared.complete_ids.next(),
        linear: d,
        named_primaries,
        eotf,
        shared: shared.clone(),
    });
    complete_descriptions.set(key, Rc::downgrade(&d));
    d
}

fn get_description2(
    shared: &Rc<Shared>,
    ld: &Rc<LinearColorDescription>,
    complete_descriptions: &CopyHashMap<CompleteDescriptionKey, Weak<ColorDescription>>,
    named_primaries: Option<NamedPrimaries>,
    eotf: Eotf,
) -> Rc<ColorDescription> {
    let key = CompleteDescriptionKey {
        linear: ld.id,
        named_primaries,
        eotf,
    };
    if let Some(d) = complete_descriptions.get(&key) {
        if let Some(d) = d.upgrade() {
            return d;
        }
        shared.dead_complete.fetch_sub(1);
    }
    let d = Rc::new(ColorDescription {
        id: shared.complete_ids.next(),
        linear: ld.clone(),
        named_primaries,
        eotf,
        shared: shared.clone(),
    });
    complete_descriptions.set(key, Rc::downgrade(&d));
    d
}
