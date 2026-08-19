use crate::git::Tree;
use crate::jay_config::EnumDef;
use crate::rules::Rule;
use crate::rules::jay_config_ipc_types::log_changed_fields;
use anyhow::Result;
use std::fmt::Write;

pub struct JayConfigIpcEnums;

impl Rule for JayConfigIpcEnums {
    fn hint(&self) -> &'static str {
        "Add new enum variants at the end of the enum"
    }

    fn check(&self, base: &Tree, head: &Tree) -> Result<Vec<String>> {
        let (base, head) = rayon::join(|| base.types(), || head.types());
        let base = &base?.enums;
        let head = &head?.enums;
        let mut violations = vec![];
        for (path, base_enum) in base {
            let Some(head_enum) = head.get(path) else {
                continue;
            };
            if let Some(violation) = compare(path, base_enum, head_enum) {
                violations.push(violation);
            }
        }
        Ok(violations)
    }
}

fn compare(path: &str, base: &EnumDef, head: &EnumDef) -> Option<String> {
    if head.variants.starts_with(&base.variants) {
        return None;
    }
    let mut msg = format!("The enum `{path}` was modified incompatibly: ",);
    let modified = base
        .variants
        .iter()
        .zip(&head.variants)
        .position(|(base, head)| base != head);
    match modified {
        Some(pos) => {
            let base = &base.variants[pos];
            let head = &head.variants[pos];
            let _ = write!(msg, "The variant `{}` was modified: ", base.name);
            if base.name != head.name {
                let _ = write!(msg, "The name was changed to `{}`", head.name);
            } else {
                log_changed_fields(&mut msg, &base.fields.fields, &head.fields.fields);
            }
        }
        None => {
            let removed: Vec<_> = base.variants[head.variants.len()..]
                .iter()
                .map(|v| format!("`{}`", v.name))
                .collect();
            let _ = write!(msg, "The variants {} were removed.", removed.join(", "));
        }
    }
    Some(msg)
}
