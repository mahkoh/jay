// backends/headless/headless_dfs_g_fuse.rs

use super::*;

pub static TARGET: Target = Target {
    path: "fuse/m_7d0cc59f634493c06b4e5a8e1f6a421fe46788b18710017e0bbb0782a20d549a.rs",
    is_global: false,
    dirs: &[
        //
        Dir {
            name: "backend",
            abstract_: false,
            parents: &["dfs_backend"],
            dirents: &[
                //
                Ent {
                    name: "monitor_fd",
                    camel: "MonitorFd",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "render_device",
                    camel: "RenderDevice",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "devs",
                    camel: "Devs",
                    ty: EntTy::View,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: Some(0),
                },
                Ent {
                    name: "backend_name",
                    camel: "BackendName",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: true,
                    predefined_key: None,
                },
                Ent {
                    name: "backend_import_environment",
                    camel: "BackendImportEnvironment",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: true,
                    predefined_key: None,
                },
                Ent {
                    name: "backend_supports_presentation_feedback",
                    camel: "BackendSupportsPresentationFeedback",
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
                disps: &[(0, 0), (2, 1)],
                map: &[4, 0, 1, 3, 5, 2],
            },
        },
        Dir {
            name: "dev",
            abstract_: false,
            parents: &[],
            dirents: &[
                //
                Ent {
                    name: "id",
                    camel: "Id",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "dev",
                    camel: "Dev",
                    ty: EntTy::Reg,
                    opt: false,
                    other: false,
                    no_timeout: false,
                    inherited: false,
                    predefined_key: None,
                },
                Ent {
                    name: "api",
                    camel: "Api",
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
                disps: &[(2, 0)],
                map: &[1, 0, 2],
            },
        },
    ],
};
