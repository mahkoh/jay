use indexmap::IndexMap;
use log::Level;
use regex::Captures;
use regex::Regex;
use regex::Replacer;
use std::env;
use std::fs::read_to_string;
use std::path::Path;
use std::sync::LazyLock;
use std::sync::OnceLock;

pub static CONFIG_DIR: OnceLock<Option<String>> = OnceLock::new();

pub fn config_dir() -> Option<&'static str> {
    macro_rules! declare_name {
        ($name:ident) => {
            const $name: &str = stringify!($name);
        };
    }
    declare_name!(JAY_CONFIG_DIR);
    declare_name!(HOME);
    declare_name!(XDG_CONFIG_HOME);

    CONFIG_DIR
        .get_or_init(|| {
            if let Ok(dir) = env::var(JAY_CONFIG_DIR) {
                Some(dir)
            } else if let Ok(xdg) = env::var(XDG_CONFIG_HOME) {
                Some(format!("{}/jay", xdg))
            } else if let Ok(home) = env::var(HOME) {
                Some(format!("{}/.config/jay", home))
            } else {
                eprintln!(
                    "None of {JAY_CONFIG_DIR}, {XDG_CONFIG_HOME}, {HOME} are set. Cannot determine config dir.",
                );
                None
            }
        })
        .as_deref()
}

#[expect(dead_code)]
fn parse_env(env: &str) -> IndexMap<String, String> {
    let def_regex = Regex::new(
        r#"(?xm)
                ^
                \s*
                (?:export\s+)?
                (?<key>[\w.-]+)
                (?:\s*=\s*?|:\s+?)
                (?<value>
                        \s*'(?:\\'|[^'])*'
                    |   \s*"(?:\\"|[^"])*"
                    |   \s*`(?:\\`|[^`])*`
                    |   [^\#\r\n]+
                )?
                \s*
                (?:\#.*)?
                $
            "#,
    )
    .unwrap();
    let string_regex = Regex::new(
        r#"(?x)
                ^
                (?:
                        '(.*)'
                    |   `(.*)`
                    |   "(.*)"
                )
                $
            "#,
    )
    .unwrap();
    let escape_regex = Regex::new(r#"\\(?<char>.)"#).unwrap();

    fn cod<'h>(c: &Captures<'h>, name: &str) -> &'h str {
        c.name(name).map(|v| v.as_str()).unwrap_or_default()
    }
    struct ValueReplacer<'a>(&'a Regex);
    impl Replacer for ValueReplacer<'_> {
        fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
            if let Some(v) = caps.get(1).or_else(|| caps.get(2)) {
                dst.push_str(v.as_str());
            } else if let Some(v) = caps.get(3) {
                let v = self.0.replace_all(v.as_str(), StringReplacer);
                dst.push_str(&v);
            }
        }
    }
    struct StringReplacer;
    impl Replacer for StringReplacer {
        fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
            let c = cod(caps, "char");
            let c = if c == "n" {
                "\n"
            } else if c == "r" {
                "\r"
            } else {
                c
            };
            dst.push_str(c);
        }
    }
    let mut res = IndexMap::default();
    for m in def_regex.captures_iter(env) {
        let key = cod(&m, "key");
        let value = cod(&m, "value");
        let value = string_regex.replace(value.trim(), ValueReplacer(&escape_regex));
        res.insert(key.to_string(), value.into_owned());
    }
    res
}

static JAY_ENV: LazyLock<IndexMap<String, String>> = LazyLock::new(|| {
    if let Some(dir) = config_dir()
        && let file = Path::new(dir).join("jay.env")
        && let Ok(v) = read_to_string(&file)
    {
        return parse_env(&v);
    }
    Default::default()
});

pub fn log_jay_env() {
    const LEVEL: Level = Level::Debug;
    if log::log_enabled!(LEVEL) {
        for (k, v) in &*JAY_ENV {
            log::log!(LEVEL, "jay.env: {k}={v}");
        }
    }
}

#[test]
fn test_parse_env() {
    let _ = parse_env("");
}

#[expect(unused)]
macro_rules! declare {
    ($name:ident: $ty:ty, $(@default = $default:expr,)? $map:expr $(,)?) => {
        #[allow(clippy::allow_attributes, non_camel_case_types)]
        pub struct $name;

        const _: () = {
            const NAME: &str = stringify!($name);

            static ENV: LazyLock<Option<String>> = LazyLock::new(|| env::var(NAME).ok());

            static RAW: LazyLock<Option<&'static str>> = LazyLock::new(|| {
                ENV.as_deref()
                    .or_else(|| JAY_ENV.get(NAME).map(|v| v.as_str()))
            });

            impl $name {
                #[allow(clippy::allow_attributes, dead_code)]
                pub fn name(self) -> &'static str {
                    NAME
                }

                #[allow(clippy::allow_attributes, dead_code)]
                pub fn as_env(self) -> impl Display {
                    fmt::from_fn(|f| {
                        f.write_str(NAME)?;
                        f.write_str("=")?;
                        if let Some(v) = *RAW {
                            f.write_str(v)?;
                        }
                        Ok(())
                    })
                }
            }

            impl Deref for $name {
                type Target = $ty;

                fn deref(&self) -> &Self::Target {

                    static COOKED: LazyLock<$ty> = {
                        LazyLock::new(|| {
                            RAW.map($map)
                            $(
                                .unwrap_or($default)
                            )?
                        })
                    };
                    &*COOKED
                }
            }
        };
    };
}

#[expect(unused)]
macro_rules! declare_str {
    ($name:ident) => {
        declare!($name: Option<&'static str>, |v| v);
    };
}

#[expect(unused)]
macro_rules! declare_bool {
    ($name:ident) => {
        declare!($name: bool, @default = false, |v| {
            v == "1" ||
            v.eq_ignore_ascii_case("true") ||
            v.eq_ignore_ascii_case("on") ||
            v.eq_ignore_ascii_case("yes")
        });
    };
}
