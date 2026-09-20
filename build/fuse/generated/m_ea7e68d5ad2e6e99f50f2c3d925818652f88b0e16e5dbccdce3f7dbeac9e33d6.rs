// ifs/wl_surface/wl_subsurface/wl_subsurface_dfs_g_fuse.rs

use super::*;

pub static TARGET: Target = Target {
    path: "fuse/m_ea7e68d5ad2e6e99f50f2c3d925818652f88b0e16e5dbccdce3f7dbeac9e33d6.rs",
    is_global: false,
    dirs: &[
        //
        Dir {
            name: "subsurface",
            abstract_: false,
            parents: &["dfs_object"],
            dirents: &[
                //
                Ent {
                    name: "parent",
                    camel: "Parent",
                    ty: EntTy::Link,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "child",
                    camel: "Child",
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
                key: 13140400953119184615,
                disps: &[(4, 0)],
                map: &[2, 3, 0, 1, 4],
            },
        },
    ],
};
