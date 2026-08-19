use crate::git::Tree;
use crate::jay_config::FieldDef;
use crate::jay_config::FieldsDef;
use crate::rules::Rule;
use anyhow::Result;
use std::fmt::Write;

pub struct JayConfigIpcTypes;

impl Rule for JayConfigIpcTypes {
    fn hint(&self) -> &'static str {
        "You cannot modify existing structs"
    }

    fn check(&self, base: &Tree, head: &Tree) -> Result<Vec<String>> {
        let (base, head) = rayon::join(|| base.types(), || head.types());
        let base = base?;
        let head = head?;
        let mut violations = vec![];
        for (path, base_struct) in &base.structs {
            let Some(head_struct) = head.structs.get(path) else {
                continue;
            };
            let subject = format!("The struct `{path}`");
            if let Some(violation) = compare(&subject, &base_struct.fields, &head_struct.fields) {
                violations.push(violation);
            }
        }
        for (path, base_enum) in &base.enums {
            let Some(head_enum) = head.enums.get(path) else {
                continue;
            };
            for base_variant in &base_enum.variants {
                let head_variant = head_enum
                    .variants
                    .iter()
                    .find(|v| v.name == base_variant.name);
                let Some(head_variant) = head_variant else {
                    continue;
                };
                let subject = format!("The variant `{}` of the enum `{path}`", base_variant.name);
                if let Some(violation) =
                    compare(&subject, &base_variant.fields, &head_variant.fields)
                {
                    violations.push(violation);
                }
            }
        }
        violations.sort();
        Ok(violations)
    }
}

fn compare(subject: &str, base: &FieldsDef, head: &FieldsDef) -> Option<String> {
    if base == head {
        return None;
    }
    let mut msg = format!("{subject} was modified: ");
    log_changed_fields(&mut msg, &base.fields, &head.fields);
    Some(msg)
}

pub fn log_changed_fields(msg: &mut String, base: &[FieldDef], head: &[FieldDef]) {
    let modified = base.iter().zip(head).position(|(base, head)| base != head);
    match modified {
        Some(pos) => {
            let base = &base[pos];
            let head = &head[pos];
            let _ = write!(msg, "The field {} was modified: ", base.name(pos));
            if base.name != head.name {
                let _ = write!(msg, "The name was changed to {}", head.name(pos));
            } else if base.ty != head.ty {
                let _ = write!(msg, "The type changed from `{}` to `{}`", base.ty, head.ty);
            } else {
                let modified = base
                    .attrs
                    .iter()
                    .zip(&head.attrs)
                    .position(|(base, head)| base != head);
                match modified {
                    Some(pos) => {
                        let base = &base.attrs[pos];
                        let head = &head.attrs[pos];
                        let _ = write!(
                            msg,
                            "The {pos}th attribute changed from `{base}` to `{head}`",
                        );
                    }
                    None => {
                        if base.attrs.len() < head.attrs.len() {
                            let _ = write!(
                                msg,
                                "The attribute `{}` was added",
                                head.attrs[base.attrs.len()],
                            );
                        } else {
                            let _ = write!(
                                msg,
                                "The attribute `{}` was removed",
                                base.attrs[head.attrs.len()],
                            );
                        }
                    }
                }
            }
        }
        None => {
            if base.len() < head.len() {
                let pos = base.len();
                let _ = write!(msg, "The field {} was added", head[pos].name(pos));
            } else {
                let pos = head.len();
                let _ = write!(msg, "The field {} was removed", base[pos].name(pos));
            }
        }
    }
}
