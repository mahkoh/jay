// object/object_dfs_g_fuse.rs

use super::*;

pub static TARGET: Target = Target {
    path: "fuse/m_28877255ddf122e7e59f2320d1b07ed8c48ad1eb24663c25f1bd9062e6c2405b.rs",
    is_global: false,
    dirs: &[
        //
        Dir {
            name: "generic_object",
            abstract_: false,
            parents: &[],
            dirents: &[
                //
                Ent {
                    name: "object_id",
                    camel: "ObjectId",
                    ty: EntTy::View,
                    opt: false,
                    other: false,
                    no_timeout: true,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "object_interface",
                    camel: "ObjectInterface",
                    ty: EntTy::View,
                    opt: false,
                    other: false,
                    no_timeout: true,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "object_version",
                    camel: "ObjectVersion",
                    ty: EntTy::View,
                    opt: false,
                    other: false,
                    no_timeout: true,
                    inherited: false,
                    predefined_key: None,
                },
            ],
            phf: PhfMap {
                key: 4557430889152533588,
                disps: &[(1, 0)],
                map: &[1, 2, 0],
            },
        },
    ],
};
