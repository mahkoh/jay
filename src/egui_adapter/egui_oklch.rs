use {
    crate::{
        cmm::cmm_eotf::Eotf,
        theme::{Color, Oklab, Oklch},
    },
    egui::{Color32, Rgba},
};

#[expect(dead_code)]
pub trait Color32Ext {
    fn to_oklab(self) -> Oklab;
    fn to_oklch(self) -> Oklch;
}

impl Color32Ext for Color32 {
    fn to_oklab(self) -> Oklab {
        let [r, g, b, a] = self.to_array();
        Color::from_srgba_premultiplied(r, g, b, a).srgb_to_oklab()
    }

    fn to_oklch(self) -> Oklch {
        self.to_oklab().to_oklch()
    }
}

impl Into<Color32> for Oklch {
    fn into(self) -> Color32 {
        self.to_oklab().into()
    }
}

impl Into<Color32> for Oklab {
    fn into(self) -> Color32 {
        let [r, g, b, a] = self.to_srgb().to_array(Eotf::Linear);
        Rgba::from_rgba_premultiplied(r, g, b, a).into()
    }
}
