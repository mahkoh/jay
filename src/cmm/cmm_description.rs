use crate::cmm::cmm_eotf::Eotf;
use crate::cmm::cmm_luminance::Luminance;
use crate::cmm::cmm_luminance::TargetLuminance;
use crate::cmm::cmm_luminance::white_balance;
use crate::cmm::cmm_manager::Shared;
use crate::cmm::cmm_primaries::NamedPrimaries;
use crate::cmm::cmm_primaries::Primaries;
use crate::cmm::cmm_render_intent::RenderIntent;
use crate::cmm::cmm_transform::ColorMatrix;
use crate::cmm::cmm_transform::Local;
use crate::cmm::cmm_transform::Xyz;
use crate::cmm::cmm_transform::bradford_adjustment;
use crate::utils::ordered_float::F64;
use jay_algorithms::triangles::triangle_contains_points;
use std::cell::OnceCell;
use std::rc::Rc;

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
    pub target_contained_in_primary: OnceCell<bool>,
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
    pub fn color_transform(&self, target: &Self, intent: RenderIntent) -> ColorMatrix {
        let mut mat = target.local_from_xyz;
        if self.luminance != target.luminance {
            mat *= white_balance(
                &self.luminance,
                &target.luminance,
                target.primaries.wp,
                intent,
            );
        }
        if self.primaries.wp != target.primaries.wp && intent.bradford_adjustment() {
            mat *= bradford_adjustment(self.primaries.wp, target.primaries.wp);
        }
        mat * self.xyz_from_local
    }

    pub fn embeds_into(&self, target: &Self, intent: RenderIntent) -> bool {
        if self.id == target.id {
            return true;
        }
        if !self.primaries.about_equal(&target.primaries) {
            return false;
        }
        if !self.luminance.embeds_into(&target.luminance, intent) {
            return false;
        }
        true
    }

    pub fn target_contained_in_primary(&self) -> bool {
        *self.target_contained_in_primary.get_or_init(|| {
            if self.target_luminance.min.0 < self.luminance.min.0
                || self.target_luminance.max.0 > self.luminance.max.0
            {
                return false;
            }
            #[rustfmt::skip]
            let extract = |p: &Primaries| [
                [p.r.0.0, p.r.1.0],
                [p.g.0.0, p.g.1.0],
                [p.b.0.0, p.b.1.0],
            ];
            triangle_contains_points(extract(&self.primaries), extract(&self.target_primaries))
        })
    }
}

impl ColorDescription {
    pub fn embeds_into(&self, target: &Self, intent: RenderIntent) -> bool {
        self.eotf == target.eotf && self.linear.embeds_into(&target.linear, intent)
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
