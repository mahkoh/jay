use {
    crate::{
        cmm::{
            cmm_eotf::Eotf,
            cmm_luminance::{Luminance, TargetLuminance, white_balance},
            cmm_manager::Shared,
            cmm_primaries::{NamedPrimaries, Primaries},
            cmm_transform::{ColorMatrix, Local, Xyz, bradford_adjustment},
        },
        utils::ordered_float::F64,
    },
    std::rc::Rc,
};

linear_ids!(LinearColorDescriptionIds, LinearColorDescriptionId, u64);
linear_ids!(ColorDescriptionIds, ColorDescriptionId, u64);

#[derive(Debug)]
pub struct LinearColorDescription {
    pub id: LinearColorDescriptionId,
    pub primaries: Primaries,
    pub xyz_from_local: ColorMatrix<Xyz, Local>,
    pub local_from_xyz: ColorMatrix<Local, Xyz>,
    pub luminance: Luminance,
    pub target_primaries: Primaries,
    pub target_luminance: TargetLuminance,
    pub max_cll: Option<F64>,
    pub max_fall: Option<F64>,
    pub(super) shared: Rc<Shared>,
}

#[derive(Debug)]
pub struct ColorDescription {
    pub id: ColorDescriptionId,
    pub linear: Rc<LinearColorDescription>,
    pub named_primaries: Option<NamedPrimaries>,
    pub eotf: Eotf,
    pub(super) shared: Rc<Shared>,
}

impl LinearColorDescription {
    pub fn color_transform(&self, target: &Self) -> ColorMatrix {
        let mut mat = target.local_from_xyz;
        if self.luminance != target.luminance {
            mat *= white_balance(&self.luminance, &target.luminance, target.primaries.wp);
        }
        if self.primaries.wp != target.primaries.wp {
            mat *= bradford_adjustment(self.primaries.wp, target.primaries.wp);
        }
        mat * self.xyz_from_local
    }

    pub fn embeds_into(&self, target: &Self) -> bool {
        if self.id == target.id {
            return true;
        }
        if self.primaries != target.primaries {
            return false;
        }
        if self.luminance != target.luminance {
            return false;
        }
        true
    }
}

impl ColorDescription {
    pub fn embeds_into(&self, target: &Self) -> bool {
        self.eotf == target.eotf && self.linear.embeds_into(&target.linear)
    }
}

impl Drop for LinearColorDescription {
    fn drop(&mut self) {
        self.shared.dead_linear.fetch_add(1);
    }
}

impl Drop for ColorDescription {
    fn drop(&mut self) {
        self.shared.dead_complete.fetch_add(1);
    }
}
