use {
    crate::{
        cmm::{
            cmm_luminance::{Luminance, white_balance},
            cmm_manager::Shared,
            cmm_primaries::{NamedPrimaries, Primaries},
            cmm_transfer_function::TransferFunction,
            cmm_transform::{ColorMatrix, Local, Xyz, bradford_adjustment},
        },
        utils::free_list::FreeList,
    },
    std::rc::Rc,
};

linear_ids!(LinearColorDescriptionIds, LinearColorDescriptionId, u64);

pub type ColorDescriptionIds = FreeList<ColorDescriptionId, 3>;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ColorDescriptionId(u32);

impl From<u32> for ColorDescriptionId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<ColorDescriptionId> for u32 {
    fn from(value: ColorDescriptionId) -> Self {
        value.0
    }
}

#[derive(Debug)]
pub struct LinearColorDescription {
    pub id: LinearColorDescriptionId,
    pub primaries: Primaries,
    pub xyz_from_local: ColorMatrix<Xyz, Local>,
    pub local_from_xyz: ColorMatrix<Local, Xyz>,
    pub luminance: Luminance,
    pub(super) shared: Rc<Shared>,
}

#[derive(Debug)]
pub struct ColorDescription {
    pub id: ColorDescriptionId,
    pub linear: Rc<LinearColorDescription>,
    #[expect(dead_code)]
    pub named_primaries: Option<NamedPrimaries>,
    pub transfer_function: TransferFunction,
    pub(super) shared: Rc<Shared>,
}

impl LinearColorDescription {
    #[expect(dead_code)]
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
}

impl Drop for LinearColorDescription {
    fn drop(&mut self) {
        self.shared.dead_linear.fetch_add(1);
    }
}

impl Drop for ColorDescription {
    fn drop(&mut self) {
        self.shared.dead_complete.fetch_add(1);
        self.shared.complete_ids.release(self.id);
    }
}
