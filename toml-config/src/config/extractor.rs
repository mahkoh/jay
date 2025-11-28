use {
    crate::{
        config::context::Context,
        toml::{
            toml_span::{Span, Spanned, SpannedExt},
            toml_value::Value,
        },
    },
    ahash::AHashSet,
    error_reporter::Report,
    indexmap::IndexMap,
    thiserror::Error,
};

pub struct Extractor<'v> {
    cx: &'v Context<'v>,
    table: &'v IndexMap<Spanned<String>, Spanned<Value>>,
    used: Vec<&'static str>,
    log_unused: bool,
    span: Span,
}

impl<'v> Extractor<'v> {
    pub fn new(
        cx: &'v Context<'v>,
        span: Span,
        table: &'v IndexMap<Spanned<String>, Spanned<Value>>,
    ) -> Self {
        Self {
            cx,
            table,
            used: Default::default(),
            log_unused: true,
            span,
        }
    }

    fn get(&mut self, name: &'static str) -> Option<&'v Spanned<Value>> {
        let v = self.table.get(name);
        if v.is_some() {
            self.used.push(name);
        }
        v
    }

    pub fn ignore_unused(&mut self) {
        self.log_unused = false;
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn extract<E: Extractable<'v>, U>(&mut self, e: E) -> Result<E::Output, Spanned<U>>
    where
        ExtractorError: Into<U>,
    {
        e.extract(self).map_err(|e| e.map(|e| e.into()))
    }

    pub fn extract_or_ignore<E: Extractable<'v>, U>(
        &mut self,
        e: E,
    ) -> Result<E::Output, Spanned<U>>
    where
        ExtractorError: Into<U>,
    {
        let res = self.extract(e);
        if res.is_err() {
            self.ignore_unused();
        }
        res
    }
}

impl Drop for Extractor<'_> {
    fn drop(&mut self) {
        if !self.log_unused {
            return;
        }
        if self.used.len() == self.table.len() {
            return;
        }
        let used: AHashSet<_> = self.used.iter().copied().collect();
        for key in self.table.keys() {
            if !used.contains(key.value.as_str()) {
                #[derive(Debug, Error)]
                #[error("Ignoring unknown key {0}")]
                struct Err<'a>(&'a str);
                let err = self.cx.error2(key.span, Err(&key.value));
                log::warn!("{}", Report::new(err));
            }
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ExtractorError {
    #[error("Missing field {0}")]
    MissingField(&'static str),
    #[error("Expected {0} but found {1}")]
    Expected(&'static str, &'static str),
    #[error("Value must fit in a u32")]
    U32,
    #[error("Value must fit in a u64")]
    U64,
    #[error("Value must fit in a i32")]
    I32,
}

pub trait Extractable<'v> {
    type Output;

    fn extract(
        self,
        extractor: &mut Extractor<'v>,
    ) -> Result<Self::Output, Spanned<ExtractorError>>;
}

impl<'v, T, F> Extractable<'v> for F
where
    F: FnOnce(&mut Extractor<'v>) -> Result<T, Spanned<ExtractorError>>,
{
    type Output = T;

    fn extract(
        self,
        extractor: &mut Extractor<'v>,
    ) -> Result<Self::Output, Spanned<ExtractorError>> {
        self(extractor)
    }
}

pub fn val(
    name: &'static str,
) -> impl for<'v> FnOnce(&mut Extractor<'v>) -> Result<Spanned<&'v Value>, Spanned<ExtractorError>>
{
    move |extractor: &mut Extractor| match extractor.get(name) {
        None => Err(ExtractorError::MissingField(name).spanned(extractor.span)),
        Some(v) => Ok(v.as_ref()),
    }
}

macro_rules! ty {
    ($f:ident, $lt:lifetime, $ty:ident, $ret:ty, $v:ident, $map:expr, $name:expr) => {
        pub fn $f(
            name: &'static str,
        ) -> impl for<$lt> FnOnce(&mut Extractor<$lt>) -> Result<Spanned<$ret>, Spanned<ExtractorError>> {
            move |extractor: &mut Extractor| {
                val(name)(extractor).and_then(|v| match v.value {
                    Value::$ty($v) => Ok($map.spanned(v.span)),
                    _ => Err(ExtractorError::Expected($name, v.value.name()).spanned(v.span)),
                })
            }
        }
    };
}

ty!(str, 'a, String, &'a str, v, v.as_str(), "a string");
ty!(int, 'a, Integer, i64, v, *v, "an integer");
// ty!(flt, 'a, Float, f64, v, *v, "a float");
ty!(bol, 'a, Boolean, bool, v, *v, "a boolean");
ty!(arr, 'a, Array, &'a [Spanned<Value>], v, &**v, "an array");
// ty!(tbl, 'a, Table, &'a IndexMap<Spanned<String>, Spanned<Value>>, v, v, "a table");

pub fn fltorint(
    name: &'static str,
) -> impl for<'a> FnOnce(&mut Extractor<'a>) -> Result<Spanned<f64>, Spanned<ExtractorError>> {
    move |extractor: &mut Extractor| {
        val(name)(extractor).and_then(|v| match *v.value {
            Value::Float(f) => Ok(f.spanned(v.span)),
            Value::Integer(i) => Ok((i as f64).spanned(v.span)),
            _ => Err(
                ExtractorError::Expected("a float or an integer", v.value.name()).spanned(v.span),
            ),
        })
    }
}

macro_rules! int {
    ($f:ident, $ty:ident, $err:ident) => {
        pub fn $f(
            name: &'static str,
        ) -> impl for<'v> FnOnce(&mut Extractor<'v>) -> Result<Spanned<$ty>, Spanned<ExtractorError>> {
            move |extractor: &mut Extractor| {
                int(name)(extractor).and_then(|v| {
                    v.value
                        .try_into()
                        .map(|n: $ty| n.spanned(v.span))
                        .map_err(|_| ExtractorError::$err.spanned(v.span))
                })
            }
        }
    };
}

int!(n32, u32, U32);
int!(n64, u64, U64);
int!(s32, i32, I32);

pub fn recover<F>(f: F) -> Recover<F> {
    Recover(f)
}

pub struct Recover<E>(E);

impl<'v, E> Extractable<'v> for Recover<E>
where
    E: Extractable<'v>,
    <E as Extractable<'v>>::Output: Default,
{
    type Output = <E as Extractable<'v>>::Output;

    fn extract(
        self,
        extractor: &mut Extractor<'v>,
    ) -> Result<Self::Output, Spanned<ExtractorError>> {
        match self.0.extract(extractor) {
            Ok(v) => Ok(v),
            Err(e) => {
                log::warn!("{}", extractor.cx.error(e));
                Ok(<E as Extractable<'v>>::Output::default())
            }
        }
    }
}

pub fn opt<F>(f: F) -> Opt<F> {
    Opt(f)
}

pub struct Opt<E>(E);

impl<'v, E> Extractable<'v> for Opt<E>
where
    E: Extractable<'v>,
{
    type Output = Option<<E as Extractable<'v>>::Output>;

    fn extract(
        self,
        extractor: &mut Extractor<'v>,
    ) -> Result<Self::Output, Spanned<ExtractorError>> {
        match self.0.extract(extractor) {
            Ok(v) => Ok(Some(v)),
            Err(e) if matches!(e.value, ExtractorError::MissingField(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

macro_rules! tuples {
    ($($idx:tt: $name:ident,)*) => {
        impl<'v, $($name,)*> Extractable<'v> for ($($name,)*)
            where $($name: Extractable<'v>,)*
        {
            type Output = ($($name::Output,)*);

            #[expect(non_snake_case)]
            fn extract(self, extractor: &mut Extractor<'v>) -> Result<Self::Output, Spanned<ExtractorError>> {
                $(
                    let $name = self.$idx.extract(extractor);
                )*
                Ok((
                    $(
                        $name?,
                    )*
                ))
            }
        }
    };
}

tuples!(0:T0,);
tuples!(0:T0,1:T1,);
tuples!(0:T0,1:T1,2:T2,);
tuples!(0:T0,1:T1,2:T2,3:T3,);
tuples!(0:T0,1:T1,2:T2,3:T3,4:T4,);
tuples!(0:T0,1:T1,2:T2,3:T3,4:T4,5:T5,);
tuples!(0:T0,1:T1,2:T2,3:T3,4:T4,5:T5,6:T6,);
tuples!(0:T0,1:T1,2:T2,3:T3,4:T4,5:T5,6:T6,7:T7,);
tuples!(0:T0,1:T1,2:T2,3:T3,4:T4,5:T5,6:T6,7:T7,8:T8,);
tuples!(0:T0,1:T1,2:T2,3:T3,4:T4,5:T5,6:T6,7:T7,8:T8,9:T9,);
