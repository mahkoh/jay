use {
    crate::{
        cmm::{
            cmm_description::{
                ColorDescription, ColorDescriptionIds, LinearColorDescription,
                LinearColorDescriptionId, LinearColorDescriptionIds,
            },
            cmm_luminance::{Luminance, TargetLuminance},
            cmm_primaries::{NamedPrimaries, Primaries},
            cmm_transfer_function::TransferFunction,
        },
        utils::{copyhashmap::CopyHashMap, numcell::NumCell, ordered_float::F64},
    },
    std::rc::{Rc, Weak},
};

pub struct ColorManager {
    linear_ids: LinearColorDescriptionIds,
    linear_descriptions: CopyHashMap<LinearDescriptionKey, Weak<LinearColorDescription>>,
    complete_descriptions: CopyHashMap<CompleteDescriptionKey, Weak<ColorDescription>>,
    shared: Rc<Shared>,
    srgb_srgb: Rc<ColorDescription>,
    srgb_linear: Rc<ColorDescription>,
    windows_scrgb: Rc<ColorDescription>,
}

#[derive(Debug, Default)]
pub(super) struct Shared {
    pub(super) dead_linear: NumCell<usize>,
    pub(super) dead_complete: NumCell<usize>,
    pub(super) complete_ids: ColorDescriptionIds,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct LinearDescriptionKey {
    primaries: Primaries,
    luminance: Luminance,
    target_primaries: Primaries,
    target_luminance: TargetLuminance,
    max_cll: Option<F64>,
    max_fall: Option<F64>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct CompleteDescriptionKey {
    linear: LinearColorDescriptionId,
    named_primaries: Option<NamedPrimaries>,
    transfer_function: TransferFunction,
}

impl ColorManager {
    pub fn new() -> Rc<Self> {
        let linear_ids = LinearColorDescriptionIds::default();
        let linear_descriptions = CopyHashMap::default();
        let complete_descriptions = CopyHashMap::default();
        let shared = Rc::new(Shared::default());
        let _ = shared.complete_ids.next();
        let srgb_srgb = get_description(
            &shared,
            &linear_descriptions,
            &complete_descriptions,
            &linear_ids,
            Some(NamedPrimaries::Srgb),
            Primaries::SRGB,
            Luminance::SRGB,
            TransferFunction::Srgb,
            Primaries::SRGB,
            Luminance::SRGB.to_target(),
            None,
            None,
        );
        let srgb_linear = get_description2(
            &shared,
            &srgb_srgb.linear,
            &complete_descriptions,
            Some(NamedPrimaries::Srgb),
            TransferFunction::Linear,
        );
        let windows_scrgb = get_description(
            &shared,
            &linear_descriptions,
            &complete_descriptions,
            &linear_ids,
            Some(NamedPrimaries::Srgb),
            Primaries::SRGB,
            Luminance::WINDOWS_SCRGB,
            TransferFunction::Linear,
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
            srgb_srgb,
            srgb_linear,
            windows_scrgb,
        })
    }

    pub fn srgb_srgb(&self) -> &Rc<ColorDescription> {
        &self.srgb_srgb
    }

    pub fn srgb_linear(&self) -> &Rc<ColorDescription> {
        &self.srgb_linear
    }

    pub fn windows_scrgb(&self) -> &Rc<ColorDescription> {
        &self.windows_scrgb
    }

    pub fn get_description(
        self: &Rc<Self>,
        named_primaries: Option<NamedPrimaries>,
        primaries: Primaries,
        luminance: Luminance,
        transfer_function: TransferFunction,
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
            transfer_function,
            target_primaries,
            target_luminance,
            max_cll,
            max_fall,
        )
    }

    pub fn get_with_tf(
        self: &Rc<Self>,
        cd: &Rc<ColorDescription>,
        transfer_function: TransferFunction,
    ) -> Rc<ColorDescription> {
        get_description2(
            &self.shared,
            &cd.linear,
            &self.complete_descriptions,
            cd.named_primaries,
            transfer_function,
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
    transfer_function: TransferFunction,
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
            return get_description2(
                shared,
                &d,
                complete_descriptions,
                named_primaries,
                transfer_function,
            );
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
        max_cll,
        max_fall,
        shared: shared.clone(),
    });
    linear_descriptions.set(key, Rc::downgrade(&d));
    let key = CompleteDescriptionKey {
        linear: d.id,
        named_primaries,
        transfer_function,
    };
    let d = Rc::new(ColorDescription {
        id: shared.complete_ids.next(),
        linear: d,
        named_primaries,
        transfer_function,
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
    transfer_function: TransferFunction,
) -> Rc<ColorDescription> {
    let key = CompleteDescriptionKey {
        linear: ld.id,
        named_primaries,
        transfer_function,
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
        transfer_function,
        shared: shared.clone(),
    });
    complete_descriptions.set(key, Rc::downgrade(&d));
    d
}
