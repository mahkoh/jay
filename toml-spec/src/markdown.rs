use {
    crate::types::{
        ArraySpec, Described, NestableTypesSpec, NumberSpec, RefOrSpec, SingleTableSpec,
        StringSpec, TableSpec, TopLevelTypeSpec, VariantSpec,
    },
    anyhow::Result,
    std::io::Write,
};

pub fn generate_markdown(types: &[(&String, &Described<TopLevelTypeSpec>)]) -> Result<()> {
    const TEMPLATE: &str = include_str!("../spec/template.md");

    let mut buf = vec![];
    buf.extend_from_slice(TEMPLATE.as_bytes());

    for (name, ty) in types {
        write_top_level_type_spec(&mut buf, name, ty)?;
    }

    std::fs::write(
        concat!(env!("CARGO_MANIFEST_DIR"), "/spec/spec.generated.md"),
        &buf,
    )?;

    Ok(())
}

fn write_top_level_type_spec(
    buf: &mut Vec<u8>,
    name: &str,
    spec: &Described<TopLevelTypeSpec>,
) -> Result<()> {
    writeln!(buf, "<a name=\"types-{name}\"></a>")?;
    writeln!(buf, "### `{name}`")?;
    writeln!(buf)?;
    writeln!(buf, "{}", spec.description.trim())?;
    writeln!(buf)?;
    match &spec.value {
        TopLevelTypeSpec::Variable { variants } => {
            writeln!(
                buf,
                "Values of this type should have one of the following forms:"
            )?;
            writeln!(buf)?;
            for variant in variants {
                write!(buf, "#### ")?;
                let name = match &variant.value {
                    VariantSpec::String(_) => "A string",
                    VariantSpec::Number(_) => "A number",
                    VariantSpec::Boolean => "A boolean",
                    VariantSpec::Array(_) => "An array",
                    VariantSpec::Table(_) => "A table",
                };
                writeln!(buf, "{name}")?;
                writeln!(buf)?;
                writeln!(buf, "{}", variant.description.trim())?;
                writeln!(buf)?;
                write_variant_spec(buf, &variant.value)?;
            }
        }
        TopLevelTypeSpec::Single(variant) => {
            let name = match &variant {
                VariantSpec::String(_) => "strings",
                VariantSpec::Number(_) => "numbers",
                VariantSpec::Boolean => "booleans",
                VariantSpec::Array(_) => "arrays",
                VariantSpec::Table(_) => "tables",
            };
            writeln!(buf, "Values of this type should be {name}.")?;
            writeln!(buf)?;
            write_variant_spec(buf, variant)?;
        }
    }
    writeln!(buf)?;
    Ok(())
}

fn write_variant_spec(buf: &mut Vec<u8>, spec: &VariantSpec) -> Result<()> {
    macro_rules! spec {
        ($v:expr) => {
            match $v {
                RefOrSpec::Ref { name } => {
                    writeln!(buf, "The value should be a [{name}](#types-{name}).")?;
                    writeln!(buf)?;
                    return Ok(());
                }
                RefOrSpec::Spec(s) => s,
            }
        };
    }
    match spec {
        VariantSpec::String(ss) => {
            let ss = spec!(ss);
            write_string_spec(buf, ss, "")?;
        }
        VariantSpec::Number(ns) => {
            let ns = spec!(ns);
            write_number_spec(buf, ns, "")?;
        }
        VariantSpec::Boolean => {}
        VariantSpec::Array(s) => {
            let s = spec!(s);
            write_array_spec(buf, s, "")?;
        }
        VariantSpec::Table(ts) => {
            let ts = spec!(ts);
            match ts {
                TableSpec::Tagged { types } => {
                    writeln!(
                        buf,
                        "This table is a tagged union. The variant is determined by the `type` field. It takes one of the following values:"
                    )?;
                    writeln!(buf)?;
                    for (name, spec) in types {
                        writeln!(buf, "- `{name}`:")?;
                        writeln!(buf)?;
                        for line in spec.description.trim().lines() {
                            writeln!(buf, "  {line}")?;
                        }
                        writeln!(buf)?;
                        write_single_table_spec(buf, &spec.value, "  ")?;
                    }
                }
                TableSpec::Single(s) => {
                    write_single_table_spec(buf, s, "")?;
                }
            }
        }
    }
    Ok(())
}

fn write_single_table_spec(buf: &mut Vec<u8>, spec: &SingleTableSpec, pad: &str) -> Result<()> {
    writeln!(buf, "{pad}The table has the following fields:")?;
    writeln!(buf)?;
    for (name, fs) in &spec.fields {
        let optional = match fs.value.required {
            true => "required",
            false => "optional",
        };
        writeln!(buf, "{pad}- `{name}` ({optional}):")?;
        writeln!(buf)?;
        for line in fs.description.trim().lines() {
            writeln!(buf, "{pad}  {line}")?;
        }
        writeln!(buf)?;
        write!(buf, "{pad}  The value of this field should be ")?;
        let spec = write_nestable_type_spec(buf, &fs.value.kind, false)?;
        writeln!(buf, ".")?;
        writeln!(buf)?;
        if let Some(spec) = spec {
            let pad = format!("{pad}  ");
            write_nestable_type_restrictions(buf, spec, &pad)?;
        }
    }
    Ok(())
}

fn write_nestable_type_spec<'a>(
    buf: &mut Vec<u8>,
    spec: &'a RefOrSpec<NestableTypesSpec>,
    plural: bool,
) -> Result<Option<&'a NestableTypesSpec>> {
    let spec = match spec {
        RefOrSpec::Ref { name } => {
            if plural {
                write!(buf, "[{name}s](#types-{name})")?;
            } else {
                write!(buf, "a [{name}](#types-{name})")?;
            }
            return Ok(None);
        }
        RefOrSpec::Spec(s) => s,
    };
    let name = match (spec, plural) {
        (NestableTypesSpec::String(_), false) => "a string",
        (NestableTypesSpec::String(_), true) => "strings",
        (NestableTypesSpec::Number(_), false) => "a number",
        (NestableTypesSpec::Number(_), true) => "numbers",
        (NestableTypesSpec::Boolean, false) => "a boolean",
        (NestableTypesSpec::Boolean, true) => "booleans",
        (NestableTypesSpec::Map(s), _) => {
            let name = match plural {
                true => "tables",
                false => "a table",
            };
            write!(buf, "{name} whose values are ")?;
            return write_nestable_type_spec(buf, &s.values, true);
        }
        (NestableTypesSpec::Array(s), _) => {
            let name = match plural {
                true => "arrays",
                false => "an array",
            };
            write!(buf, "{name} of ")?;
            return write_nestable_type_spec(buf, &s.items, true);
        }
    };
    write!(buf, "{name}")?;
    Ok(Some(spec))
}

fn write_nestable_type_restrictions(
    buf: &mut Vec<u8>,
    spec: &NestableTypesSpec,
    pad: &str,
) -> Result<()> {
    match spec {
        NestableTypesSpec::String(s) => write_string_spec(buf, s, pad),
        NestableTypesSpec::Number(s) => write_number_spec(buf, s, pad),
        NestableTypesSpec::Boolean => Ok(()),
        NestableTypesSpec::Array(_) => Ok(()),
        NestableTypesSpec::Map(_) => Ok(()),
    }
}

fn write_string_spec(buf: &mut Vec<u8>, spec: &StringSpec, pad: &str) -> Result<()> {
    if let Some(values) = &spec.values {
        writeln!(
            buf,
            "{pad}The string should have one of the following values:"
        )?;
        writeln!(buf)?;
        for value in values {
            writeln!(buf, "{pad}- `{}`:", value.value.value)?;
            writeln!(buf)?;
            for line in value.description.lines() {
                writeln!(buf, "{pad}  {line}")?;
            }
            writeln!(buf)?;
        }
        writeln!(buf)?;
    }
    Ok(())
}

fn write_array_spec(buf: &mut Vec<u8>, spec: &ArraySpec, pad: &str) -> Result<()> {
    write!(buf, "{pad}Each element of this array should be ")?;
    let spec = write_nestable_type_spec(buf, &spec.items, false)?;
    writeln!(buf, ".")?;
    writeln!(buf)?;
    if let Some(spec) = spec {
        write_nestable_type_restrictions(buf, spec, pad)?;
    }
    Ok(())
}

fn write_number_spec(buf: &mut Vec<u8>, spec: &NumberSpec, pad: &str) -> Result<()> {
    if spec.integer_only {
        writeln!(buf, "{pad}The numbers should be integers.")?;
        writeln!(buf)?;
    }
    if let Some(minimum) = spec.minimum {
        let greater = match spec.exclusive_minimum {
            true => "strictly greater than",
            false => "greater than or equal to",
        };
        writeln!(buf, "{pad}The numbers should be {greater} {minimum}.")?;
        writeln!(buf)?;
    }
    Ok(())
}
