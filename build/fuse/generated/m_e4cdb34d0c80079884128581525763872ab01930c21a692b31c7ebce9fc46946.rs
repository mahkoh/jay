// ifs/wl_surface/zwp_input_popup_surface_v2/zwp_input_popup_surface_v2_dfs_g_fuse.rs

use super::*;

pub static TARGET: Target = Target {
    path: "fuse/m_e4cdb34d0c80079884128581525763872ab01930c21a692b31c7ebce9fc46946.rs",
    is_global: false,
    dirs: &[
        //
        Dir {
            name: "input_popup",
            abstract_: false,
            parents: &["dfs_object"],
            dirents: &[
                //
                Ent {
                    name: "positioning_scheduled",
                    camel: "PositioningScheduled",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "was_on_screen",
                    camel: "WasOnScreen",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "surface",
                    camel: "Surface",
                    ty: EntTy::Link,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "input_method",
                    camel: "InputMethod",
                    ty: EntTy::Link,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "object_id",
                    camel: "ObjectId",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: true,
                    predefined_key: None,
                },
                Ent {
                    name: "object_interface",
                    camel: "ObjectInterface",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: true,
                    predefined_key: None,
                },
                Ent {
                    name: "object_version",
                    camel: "ObjectVersion",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: true,
                    predefined_key: None,
                },
            ],
            phf: PhfMap {
                key: 4557430889152533588,
                disps: &[(2, 0), (5, 2)],
                map: &[6, 5, 1, 0, 3, 4, 2],
            },
        },
    ],
};
