use {
    crate::{
        client::Client,
        cmm::{
            cmm_description::ColorDescription, cmm_primaries::NamedPrimaries,
            cmm_transfer_function::TransferFunction,
        },
        ifs::color_management::{
            MIN_LUM_MUL, PRIMARIES_ADOBE_RGB, PRIMARIES_BT2020, PRIMARIES_CIE1931_XYZ,
            PRIMARIES_DCI_P3, PRIMARIES_DISPLAY_P3, PRIMARIES_GENERIC_FILM, PRIMARIES_MUL,
            PRIMARIES_NTSC, PRIMARIES_PAL, PRIMARIES_PAL_M, PRIMARIES_SRGB,
            TRANSFER_FUNCTION_BT1886, TRANSFER_FUNCTION_EXT_LINEAR, TRANSFER_FUNCTION_GAMMA22,
            TRANSFER_FUNCTION_GAMMA28, TRANSFER_FUNCTION_LOG_100, TRANSFER_FUNCTION_LOG_316,
            TRANSFER_FUNCTION_ST240, TRANSFER_FUNCTION_ST428, TRANSFER_FUNCTION_ST2084_PQ,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::ordered_float::F64,
        wire::{WpImageDescriptionInfoV1Id, wp_image_description_info_v1::*},
    },
    std::{convert::Infallible, rc::Rc},
    uapi::OwnedFd,
};

pub struct WpImageDescriptionInfoV1 {
    pub id: WpImageDescriptionInfoV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl WpImageDescriptionInfoV1 {
    pub fn send_description(&self, d: &ColorDescription) {
        let tf = match d.transfer_function {
            TransferFunction::Linear => TRANSFER_FUNCTION_EXT_LINEAR,
            TransferFunction::St2084Pq => TRANSFER_FUNCTION_ST2084_PQ,
            TransferFunction::Bt1886 => TRANSFER_FUNCTION_BT1886,
            TransferFunction::Gamma22 => TRANSFER_FUNCTION_GAMMA22,
            TransferFunction::Gamma28 => TRANSFER_FUNCTION_GAMMA28,
            TransferFunction::St240 => TRANSFER_FUNCTION_ST240,
            TransferFunction::Log100 => TRANSFER_FUNCTION_LOG_100,
            TransferFunction::Log316 => TRANSFER_FUNCTION_LOG_316,
            TransferFunction::St428 => TRANSFER_FUNCTION_ST428,
        };
        self.send_primaries(&d.linear.primaries);
        if let Some(n) = d.named_primaries {
            let n = match n {
                NamedPrimaries::Srgb => PRIMARIES_SRGB,
                NamedPrimaries::PalM => PRIMARIES_PAL_M,
                NamedPrimaries::Pal => PRIMARIES_PAL,
                NamedPrimaries::Ntsc => PRIMARIES_NTSC,
                NamedPrimaries::GenericFilm => PRIMARIES_GENERIC_FILM,
                NamedPrimaries::Bt2020 => PRIMARIES_BT2020,
                NamedPrimaries::Cie1931Xyz => PRIMARIES_CIE1931_XYZ,
                NamedPrimaries::DciP3 => PRIMARIES_DCI_P3,
                NamedPrimaries::DisplayP3 => PRIMARIES_DISPLAY_P3,
                NamedPrimaries::AdobeRgb => PRIMARIES_ADOBE_RGB,
            };
            self.send_primaries_named(n);
        }
        self.send_tf_named(tf);
        self.send_luminances(&d.linear.luminance);
        self.send_target_primaries(&d.linear.target_primaries);
        self.send_target_luminances(&d.linear.target_luminance);
        if let Some(max_cll) = d.linear.max_cll {
            self.send_target_max_cll(max_cll.0);
        }
        if let Some(max_fall) = d.linear.max_fall {
            self.send_target_max_fall(max_fall.0);
        }
        self.send_done();
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    #[expect(dead_code)]
    pub fn send_ic_file(&self, file: &Rc<OwnedFd>, size: usize) {
        self.client.event(IccFile {
            self_id: self.id,
            icc: file.clone(),
            icc_size: size as _,
        });
    }

    pub fn send_primaries(&self, p: &crate::cmm::cmm_primaries::Primaries) {
        let map = |c: F64| (c.0 * PRIMARIES_MUL) as i32;
        self.client.event(Primaries {
            self_id: self.id,
            r_x: map(p.r.0),
            r_y: map(p.r.1),
            g_x: map(p.g.0),
            g_y: map(p.g.1),
            b_x: map(p.b.0),
            b_y: map(p.b.1),
            w_x: map(p.wp.0),
            w_y: map(p.wp.1),
        });
    }

    pub fn send_primaries_named(&self, primaries: u32) {
        self.client.event(PrimariesNamed {
            self_id: self.id,
            primaries,
        });
    }

    #[expect(dead_code)]
    pub fn send_tf_power(&self, eexp: f64) {
        self.client.event(TfPower {
            self_id: self.id,
            eexp: (eexp * 10_000.0) as u32,
        });
    }

    pub fn send_tf_named(&self, tf: u32) {
        self.client.event(TfNamed {
            self_id: self.id,
            tf,
        });
    }

    pub fn send_luminances(&self, l: &crate::cmm::cmm_luminance::Luminance) {
        self.client.event(Luminances {
            self_id: self.id,
            min_lum: (l.min.0 * MIN_LUM_MUL) as u32,
            max_lum: l.max.0 as _,
            reference_lum: l.white.0 as _,
        });
    }

    pub fn send_target_primaries(&self, p: &crate::cmm::cmm_primaries::Primaries) {
        let map = |c: F64| (c.0 * PRIMARIES_MUL) as i32;
        self.client.event(TargetPrimaries {
            self_id: self.id,
            r_x: map(p.r.0),
            r_y: map(p.r.1),
            g_x: map(p.g.0),
            g_y: map(p.g.1),
            b_x: map(p.b.0),
            b_y: map(p.b.1),
            w_x: map(p.wp.0),
            w_y: map(p.wp.1),
        });
    }

    pub fn send_target_luminances(&self, l: &crate::cmm::cmm_luminance::TargetLuminance) {
        self.client.event(TargetLuminance {
            self_id: self.id,
            min_lum: (l.min.0 * MIN_LUM_MUL) as u32,
            max_lum: l.max.0 as _,
        });
    }

    pub fn send_target_max_cll(&self, max_cll: f64) {
        self.client.event(TargetMaxCll {
            self_id: self.id,
            max_cll: max_cll as _,
        });
    }

    pub fn send_target_max_fall(&self, max_fall: f64) {
        self.client.event(TargetMaxFall {
            self_id: self.id,
            max_fall: max_fall as _,
        });
    }
}

impl WpImageDescriptionInfoV1RequestHandler for WpImageDescriptionInfoV1 {
    type Error = Infallible;
}

object_base! {
    self = WpImageDescriptionInfoV1;
    version = self.version;
}

impl Object for WpImageDescriptionInfoV1 {}

simple_add_obj!(WpImageDescriptionInfoV1);
