// state/state_dfs_g_fuse.rs

use super::*;

pub static TARGET: Target = Target {
    path: "fuse/m_8563e4e637c55ed8bf14a6455be558a3271e5c626f37595a04b66b07ff35056e.rs",
    is_global: false,
    dirs: &[
        //
        Dir {
            name: "root",
            abstract_: false,
            parents: &[],
            dirents: &[
                //
                Ent {
                    name: "version",
                    camel: "Version",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
            ],
            phf: PhfMap {
                key: 4557430889152533588,
                disps: &[(0, 0)],
                map: &[0],
            },
        },
    ],
};
