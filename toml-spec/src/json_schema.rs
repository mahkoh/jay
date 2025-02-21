use {
    crate::types::{
        ArraySpec, Described, MapSpec, NestableTypesSpec, NumberSpec, RefOrSpec, SingleTableSpec,
        StringSpec, TableSpec, TopLevelTypeSpec, VariantSpec,
    },
    anyhow::Result,
    serde_json::{Map, Value, json},
};

pub fn generate_json_schema(
    types_sorted: &[(&String, &Described<TopLevelTypeSpec>)],
) -> Result<()> {
    let mut types = Map::new();
    for (name, ty) in types_sorted {
        types.insert(name.to_string(), create_top_level_schema(ty));
    }

    let json = json!({
        "$id": "jay_toml_schema",
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/Config",
        "$defs": types,
    });

    let json = serde_json::to_string_pretty(&json).unwrap();

    std::fs::write(
        concat!(env!("CARGO_MANIFEST_DIR"), "/spec/spec.generated.json"),
        json.as_bytes(),
    )?;

    Ok(())
}

fn create_top_level_schema(spec: &Described<TopLevelTypeSpec>) -> Value {
    match &spec.value {
        TopLevelTypeSpec::Variable { variants } => {
            let mut cases = vec![];
            for variant in variants {
                cases.push(create_variant_schema(&variant.description, &variant.value));
            }
            json!({
                "description": spec.description,
                "anyOf": cases,
            })
        }
        TopLevelTypeSpec::Single(variant) => create_variant_schema(&spec.description, variant),
    }
}

fn create_variant_schema(description: &str, spec: &VariantSpec) -> Value {
    macro_rules! spec {
        ($v:expr) => {
            match $v {
                RefOrSpec::Ref { name } => return create_ref_spec(description, name),
                RefOrSpec::Spec(s) => s,
            }
        };
    }
    match spec {
        VariantSpec::String(ss) => {
            let ss = spec!(ss);
            create_string_spec(description, ss)
        }
        VariantSpec::Number(ns) => {
            let ns = spec!(ns);
            create_number_spec(description, ns)
        }
        VariantSpec::Boolean => create_boolean_spec(description),
        VariantSpec::Array(s) => {
            let s = spec!(s);
            create_array_spec(description, s)
        }
        VariantSpec::Table(ts) => {
            let ts = spec!(ts);
            match ts {
                TableSpec::Tagged { types } => {
                    let mut variants = vec![];
                    for (name, ty) in types {
                        variants.push(create_single_table_spec(
                            &ty.description,
                            &ty.value,
                            Some(name),
                        ));
                    }
                    json!({
                        "description": description,
                        "anyOf": variants,
                    })
                }
                TableSpec::Single(s) => create_single_table_spec(description, s, None),
            }
        }
    }
}

fn create_single_table_spec(
    description: &str,
    spec: &SingleTableSpec,
    type_: Option<&str>,
) -> Value {
    let mut properties = Map::new();
    let mut required = vec![];
    if let Some(type_) = type_ {
        properties.insert("type".into(), json!({ "const": type_ }));
        required.push("type".into());
    }
    for (key, val) in &spec.fields {
        properties.insert(
            key.into(),
            create_nestable_type_spec(&val.description, &val.value.kind),
        );
        if val.value.required {
            required.push(key.to_string());
        }
    }
    json!({
        "description": description,
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

fn create_ref_spec(description: &str, name: &str) -> Value {
    let path = format!("#/$defs/{name}");
    json!({
        "description": description,
        "$ref": path,
    })
}

fn create_nestable_type_spec(description: &str, spec: &RefOrSpec<NestableTypesSpec>) -> Value {
    let spec = match spec {
        RefOrSpec::Ref { name } => return create_ref_spec(description, name),
        RefOrSpec::Spec(s) => s,
    };
    match spec {
        NestableTypesSpec::String(s) => create_string_spec(description, s),
        NestableTypesSpec::Number(s) => create_number_spec(description, s),
        NestableTypesSpec::Boolean => create_boolean_spec(description),
        NestableTypesSpec::Array(s) => create_array_spec(description, s),
        NestableTypesSpec::Map(s) => create_map_spec(description, s),
    }
}

fn create_map_spec(description: &str, spec: &MapSpec) -> Value {
    json!({
        "description": description,
        "type": "object",
        "additionalProperties": create_nestable_type_spec("", &spec.values),
    })
}

fn create_string_spec(description: &str, spec: &StringSpec) -> Value {
    let mut res = Map::new();
    res.insert("type".into(), json!("string"));
    res.insert("description".into(), json!(description));
    if let Some(values) = &spec.values {
        let strings: Vec<_> = values.iter().map(|v| &v.value.value).collect();
        res.insert("enum".into(), json!(strings));
    }
    res.into()
}

fn create_array_spec(description: &str, spec: &ArraySpec) -> Value {
    json!({
        "type": "array",
        "description": description,
        "items": create_nestable_type_spec("", &spec.items),
    })
}

fn create_number_spec(description: &str, spec: &NumberSpec) -> Value {
    let ty = match spec.integer_only {
        true => "integer",
        false => "number",
    };
    let mut res = Map::new();
    res.insert("type".into(), json!(ty));
    res.insert("description".into(), json!(description));
    if let Some(minimum) = spec.minimum {
        let key = match spec.exclusive_minimum {
            true => "exclusiveMinimum",
            false => "minimum",
        };
        res.insert(key.into(), json!(minimum));
    }
    res.into()
}

fn create_boolean_spec(description: &str) -> Value {
    json!({"type": "boolean", "description": description})
}
