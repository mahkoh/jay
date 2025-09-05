mod matrices {
    use crate::{cmm::cmm_primaries::Primaries, utils::ordered_float::F64};

    fn check(primaries: Primaries, expected: [[f64; 4]; 3]) {
        let (ltg, gtl) = primaries.matrices();
        println!("{:#?}", ltg);
        assert!((ltg.0[0][0].0 - expected[0][0]).abs() < 0.001);
        assert!((ltg.0[0][1].0 - expected[0][1]).abs() < 0.001);
        assert!((ltg.0[0][2].0 - expected[0][2]).abs() < 0.001);
        assert!((ltg.0[0][3].0 - expected[0][3]).abs() < 0.001);
        assert!((ltg.0[1][0].0 - expected[1][0]).abs() < 0.001);
        assert!((ltg.0[1][1].0 - expected[1][1]).abs() < 0.001);
        assert!((ltg.0[1][2].0 - expected[1][2]).abs() < 0.001);
        assert!((ltg.0[1][3].0 - expected[1][3]).abs() < 0.001);
        assert!((ltg.0[2][0].0 - expected[2][0]).abs() < 0.001);
        assert!((ltg.0[2][1].0 - expected[2][1]).abs() < 0.001);
        assert!((ltg.0[2][2].0 - expected[2][2]).abs() < 0.001);
        assert!((ltg.0[2][3].0 - expected[2][3]).abs() < 0.001);
        let roundtrip = gtl * ltg;
        assert!((roundtrip.0[0][0].0 - 1.0).abs() < 0.001);
        assert!((roundtrip.0[0][1].0 - 0.0).abs() < 0.001);
        assert!((roundtrip.0[0][2].0 - 0.0).abs() < 0.001);
        assert!((roundtrip.0[0][3].0 - 0.0).abs() < 0.001);
        assert!((roundtrip.0[1][0].0 - 0.0).abs() < 0.001);
        assert!((roundtrip.0[1][1].0 - 1.0).abs() < 0.001);
        assert!((roundtrip.0[1][2].0 - 0.0).abs() < 0.001);
        assert!((roundtrip.0[1][3].0 - 0.0).abs() < 0.001);
        assert!((roundtrip.0[2][0].0 - 0.0).abs() < 0.001);
        assert!((roundtrip.0[2][1].0 - 0.0).abs() < 0.001);
        assert!((roundtrip.0[2][2].0 - 1.0).abs() < 0.001);
        assert!((roundtrip.0[2][3].0 - 0.0).abs() < 0.001);
    }

    #[test]
    fn srgb() {
        check(
            Primaries::SRGB,
            [
                [0.4124564, 0.3575761, 0.1804375, 0.0],
                [0.2126729, 0.7151522, 0.0721750, 0.0],
                [0.0193339, 0.1191920, 0.9503041, 0.0],
            ],
        );
    }

    #[test]
    fn cie1931_xyz() {
        check(
            Primaries::CIE1931_XYZ,
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
        );
    }

    #[test]
    fn adobe_rgb() {
        check(
            Primaries::ADOBE_RGB,
            [
                [0.5767309, 0.1855540, 0.1881852, 0.0],
                [0.2973769, 0.6273491, 0.0752741, 0.0],
                [0.0270343, 0.0706872, 0.9911085, 0.0],
            ],
        );
    }

    #[test]
    fn apple_rgb() {
        check(
            Primaries {
                r: (F64(0.625), F64(0.34)),
                g: (F64(0.28), F64(0.595)),
                b: (F64(0.155), F64(0.07)),
                wp: (F64(0.31271), F64(0.32902)),
            },
            [
                [0.4497288, 0.3162486, 0.1844926, 0.0],
                [0.2446525, 0.6720283, 0.0833192, 0.0],
                [0.0251848, 0.1411824, 0.9224628, 0.0],
            ],
        );
    }

    #[test]
    fn bt2020() {
        check(
            Primaries::BT2020,
            [
                [0.636958, 0.144617, 0.168881, 0.0],
                [0.262700, 0.677998, 0.059302, 0.0],
                [0.000000, 0.028073, 1.060985, 0.0],
            ],
        );
    }

    #[test]
    fn pal() {
        check(
            Primaries::PAL,
            [
                [0.4306190, 0.3415419, 0.1783091, 0.0],
                [0.2220379, 0.7066384, 0.0713236, 0.0],
                [0.0201853, 0.1295504, 0.9390944, 0.0],
            ],
        );
    }

    #[test]
    fn dci_p3() {
        check(
            Primaries::DCI_P3,
            [
                [0.445170, 0.277134, 0.172283, 0.0],
                [0.209492, 0.721595, 0.068913, 0.0],
                [-0.000000, 0.047061, 0.907355, 0.0],
            ],
        );
    }

    #[test]
    fn display_p3() {
        check(
            Primaries::DISPLAY_P3,
            [
                [0.486571, 0.265668, 0.198217, 0.0],
                [0.228975, 0.691739, 0.079287, 0.0],
                [-0.000000, 0.045113, 1.043944, 0.0],
            ],
        );
    }
}

mod transforms {
    use crate::cmm::{
        cmm_eotf::Eotf, cmm_luminance::Luminance, cmm_manager::ColorManager,
        cmm_primaries::Primaries,
    };

    fn check(p1: Primaries, p2: Primaries, expected: [[f64; 4]; 3]) {
        let manager = ColorManager::new();
        let d = |p| {
            manager.get_description(
                None,
                p,
                Luminance::SRGB,
                Eotf::Linear,
                p,
                Luminance::SRGB.to_target(),
                None,
                None,
            )
        };
        let d1 = d(p1);
        let d2 = d(p2);
        let m = d1.linear.color_transform(&d2.linear);
        println!("{:#?}", m);
        assert!((m.0[0][0].0 - expected[0][0]).abs() < 0.001);
        assert!((m.0[0][1].0 - expected[0][1]).abs() < 0.001);
        assert!((m.0[0][2].0 - expected[0][2]).abs() < 0.001);
        assert!((m.0[0][3].0 - expected[0][3]).abs() < 0.001);
        assert!((m.0[1][0].0 - expected[1][0]).abs() < 0.001);
        assert!((m.0[1][1].0 - expected[1][1]).abs() < 0.001);
        assert!((m.0[1][2].0 - expected[1][2]).abs() < 0.001);
        assert!((m.0[1][3].0 - expected[1][3]).abs() < 0.001);
        assert!((m.0[2][0].0 - expected[2][0]).abs() < 0.001);
        assert!((m.0[2][1].0 - expected[2][1]).abs() < 0.001);
        assert!((m.0[2][2].0 - expected[2][2]).abs() < 0.001);
        assert!((m.0[2][3].0 - expected[2][3]).abs() < 0.001);
    }

    #[test]
    fn srgb_to_bt2020() {
        check(
            Primaries::SRGB,
            Primaries::BT2020,
            [
                [0.627404, 0.329283, 0.043313, 0.0],
                [0.069097, 0.919540, 0.011362, 0.0],
                [0.016391, 0.088013, 0.895595, 0.0],
            ],
        )
    }

    #[test]
    fn bt2020_to_srgb() {
        check(
            Primaries::BT2020,
            Primaries::SRGB,
            [
                [1.660491, -0.587641, -0.072850, 0.0],
                [-0.124550, 1.132900, -0.008349, 0.0],
                [-0.018151, -0.100579, 1.118730, 0.0],
            ],
        )
    }

    #[test]
    fn srgb_to_dci_p3() {
        check(
            Primaries::SRGB,
            Primaries::DCI_P3,
            [
                [0.868580, 0.128919, 0.002501, 0.0],
                [0.034540, 0.961811, 0.003648, 0.0],
                [0.016771, 0.071040, 0.912189, 0.0],
            ],
        )
    }
}
