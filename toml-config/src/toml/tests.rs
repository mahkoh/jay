use {
    crate::{
        config::error::SpannedError,
        toml::{
            toml_parser::{parse, ErrorHandler, ParserError},
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    bstr::{BStr, ByteSlice},
    std::{
        convert::Infallible,
        os::unix::ffi::OsStrExt,
        panic::{catch_unwind, AssertUnwindSafe},
        str::FromStr,
    },
    walkdir::WalkDir,
};

#[test]
fn test() {
    let mut have_failures = false;
    let mut num = 0;
    for path in WalkDir::new("./toml-test/tests/valid") {
        let path = path.unwrap();
        if let Some(prefix) = path.path().as_os_str().as_bytes().strip_suffix(b".toml") {
            num += 1;
            let res = catch_unwind(AssertUnwindSafe(|| {
                have_failures |= run_test(prefix.as_bstr());
            }));
            if res.is_err() {
                eprintln!("panic while running {}", prefix.as_bstr());
            }
        }
    }
    if have_failures {
        panic!("There were test failures");
    }
    eprintln!("ran {num} tests");
}

fn run_test(prefix: &BStr) -> bool {
    let toml = std::fs::read(&format!("{}.toml", prefix)).unwrap();
    let json = std::fs::read_to_string(&format!("{}.json", prefix)).unwrap();

    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    let json_as_toml = json_to_value(json);
    let toml = match parse(toml.as_bytes(), &NoErrorHandler(prefix, &toml)) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("toml could not be parsed in test {}", prefix);
            NoErrorHandler(prefix, &toml).handle(e);
            return true;
        }
    };

    if toml != json_as_toml {
        eprintln!("toml and json differ in test {}", prefix);
        eprintln!("toml: {:#?}", toml);
        eprintln!("json: {:#?}", json_as_toml);
        true
    } else {
        false
    }
}

fn json_to_value(json: serde_json::Value) -> Spanned<Value> {
    let span = Span { lo: 0, hi: 0 };
    let val = match json {
        serde_json::Value::String(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::Null
        | serde_json::Value::Bool(_) => panic!("Unexpected type"),
        serde_json::Value::Array(v) => Value::Array(v.into_iter().map(json_to_value).collect()),
        serde_json::Value::Object(v) => {
            if v.len() == 2 && v.contains_key("type") && v.contains_key("value") {
                let ty = v.get("type").unwrap().as_str().unwrap();
                let val = v.get("value").unwrap().as_str().unwrap();
                match ty {
                    "string" => Value::String(val.to_owned()),
                    "integer" => Value::Integer(i64::from_str(val).unwrap()),
                    "float" => Value::Float(f64::from_str(val).unwrap()),
                    "bool" => Value::Boolean(bool::from_str(val).unwrap()),
                    _ => panic!("unexpected type {}", ty),
                }
            } else {
                Value::Table(
                    v.into_iter()
                        .map(|(k, v)| (k.spanned(span), json_to_value(v)))
                        .collect(),
                )
            }
        }
    };
    val.spanned(span)
}

struct NoErrorHandler<'a>(&'a BStr, &'a [u8]);

impl<'a> ErrorHandler for NoErrorHandler<'a> {
    fn handle(&self, err: Spanned<ParserError>) {
        eprintln!(
            "{}: An error occurred during validation: {}",
            self.0,
            SpannedError {
                input: self.1.into(),
                span: err.span,
                cause: Some(err.value),
            }
        );
    }

    fn redefinition(&self, err: Spanned<ParserError>, prev: Span) {
        eprintln!(
            "{}: Redefinition: {}",
            self.0,
            SpannedError {
                input: self.1.into(),
                span: err.span,
                cause: Some(err.value),
            }
        );
        eprintln!(
            "{}: Previous: {}",
            self.0,
            SpannedError {
                input: self.1.into(),
                span: prev,
                cause: None::<Infallible>,
            }
        );
    }
}
