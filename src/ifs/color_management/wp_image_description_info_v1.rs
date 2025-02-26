use {
    crate::{
        client::Client,
        ifs::color_management::consts::{PRIMARIES_SRGB, TRANSFER_FUNCTION_SRGB},
        leaks::Tracker,
        object::{Object, Version},
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
    pub fn send_srgb(&self) {
        let red = [0.64, 0.33];
        let green = [0.3, 0.6];
        let blue = [0.15, 0.06];
        let white = [0.3127, 0.3290];
        self.send_primaries(red, green, blue, white);
        self.send_primaries_named(PRIMARIES_SRGB);
        self.send_tf_named(TRANSFER_FUNCTION_SRGB);
        self.send_luminances(0.2, 80.0, 80.0);
        self.send_target_primaries(red, green, blue, white);
        self.send_target_luminances(0.2, 80.0);
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

    pub fn send_primaries(&self, r: [f64; 2], g: [f64; 2], b: [f64; 2], w: [f64; 2]) {
        let map = |c: f64| (c * 1_000_000.0) as i32;
        self.client.event(Primaries {
            self_id: self.id,
            r_x: map(r[0]),
            r_y: map(r[1]),
            g_x: map(g[0]),
            g_y: map(g[1]),
            b_x: map(b[0]),
            b_y: map(b[1]),
            w_x: map(w[0]),
            w_y: map(w[1]),
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

    pub fn send_luminances(&self, min_lum: f64, max_lum: f64, reference_lum: f64) {
        self.client.event(Luminances {
            self_id: self.id,
            min_lum: (min_lum * 10_000.0) as u32,
            max_lum: max_lum as _,
            reference_lum: reference_lum as _,
        });
    }

    pub fn send_target_primaries(&self, r: [f64; 2], g: [f64; 2], b: [f64; 2], w: [f64; 2]) {
        let map = |c: f64| (c * 1_000_000.0) as i32;
        self.client.event(TargetPrimaries {
            self_id: self.id,
            r_x: map(r[0]),
            r_y: map(r[1]),
            g_x: map(g[0]),
            g_y: map(g[1]),
            b_x: map(b[0]),
            b_y: map(b[1]),
            w_x: map(w[0]),
            w_y: map(w[1]),
        });
    }

    pub fn send_target_luminances(&self, min_lum: f64, max_lum: f64) {
        self.client.event(TargetLuminance {
            self_id: self.id,
            min_lum: (min_lum * 10_000.0) as u32,
            max_lum: max_lum as _,
        });
    }

    #[expect(dead_code)]
    pub fn send_target_max_cll(&self, max_cll: f64) {
        self.client.event(TargetMaxCll {
            self_id: self.id,
            max_cll: max_cll as _,
        });
    }

    #[expect(dead_code)]
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
