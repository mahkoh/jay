use crate::git::Tree;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use derivative::Derivative;
use parking_lot::Mutex;
use proc_macro2::TokenStream;
use quote::ToTokens;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use syn::Attribute;
use syn::Ident;
use syn::Item;
use syn::Path;
use syn::Token;
use syn::Type;
use syn::Visibility;
use syn::parenthesized;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;

const SOURCE_DIR: &str = "jay-config/src";
const ROOT_FILE: &str = "jay-config/src/lib.rs";
const CRATE_NAME: &str = "jay_config";

#[derive(Default)]
pub struct Types {
    pub enums: BTreeMap<String, EnumDef>,
    pub structs: BTreeMap<String, StructDef>,
}

pub struct EnumDef {
    pub variants: Vec<VariantDef>,
}

#[derive(Derivative, Eq, Debug)]
#[derivative(PartialEq)]
pub struct VariantDef {
    pub name: String,
    pub fields: FieldsDef,
}

pub struct StructDef {
    pub fields: FieldsDef,
}

#[derive(Derivative, Eq, Debug)]
#[derivative(PartialEq)]
pub struct FieldsDef {
    #[derivative(PartialEq = "ignore")]
    pub kind: FieldsKind,
    pub fields: Vec<FieldDef>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum FieldsKind {
    Struct,
    Tuple,
    Unit,
}

#[derive(Derivative, Eq, Debug)]
#[derivative(PartialEq)]
pub struct FieldDef {
    pub name: Option<String>,
    pub ty: String,
    pub attrs: Vec<String>,
}

impl FieldDef {
    pub fn name(&self, pos: usize) -> impl Display {
        fmt::from_fn(move |f| match &self.name {
            Some(n) => write!(f, "`{n}`"),
            None => write!(f, "{pos}"),
        })
    }
}

impl Display for FieldDef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for attr in &self.attrs {
            write!(f, "{attr} ")?;
        }
        if let Some(name) = &self.name {
            write!(f, "{name}: ")?;
        }
        write!(f, "{}", self.ty)
    }
}

impl Display for FieldsDef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let fields: Vec<_> = self.fields.iter().map(|f| f.to_string()).collect();
        match self.kind {
            FieldsKind::Unit => write!(f, "(no fields)"),
            FieldsKind::Tuple => write!(f, "({})", fields.join(", ")),
            FieldsKind::Struct if fields.is_empty() => write!(f, "{{}}"),
            FieldsKind::Struct => write!(f, "{{ {} }}", fields.join(", ")),
        }
    }
}

pub fn collect(tree: &Tree) -> Result<Types> {
    let collector = Collector {
        tree,
        files: Mutex::new(tree.files(SOURCE_DIR)?.into_iter().collect()),
        types: Default::default(),
    };
    collector.visit_file(ROOT_FILE, SOURCE_DIR, CRATE_NAME)?;
    Ok(collector.types.into_inner())
}

struct Collector<'a> {
    tree: &'a Tree,
    files: Mutex<HashSet<String>>,
    types: Mutex<Types>,
}

impl Collector<'_> {
    fn visit_file(&self, file: &str, dir: &str, path: &str) -> Result<()> {
        let contents = self.tree.read(file)?;
        let parsed = syn::parse_file(&contents)
            .with_context(|| format!("could not parse `{}` in {}", file, self.tree.label()))?;
        self.visit_items(&parsed.items, dir, path)
            .with_context(|| format!("could not process `{}` in {}", file, self.tree.label()))
    }

    fn visit_items(&self, items: &[Item], dir: &str, path: &str) -> Result<()> {
        let mut files = vec![];
        for item in items {
            match item {
                Item::Mod(item) => {
                    let dir = format!("{dir}/{}", item.ident);
                    let path = format!("{path}::{}", item.ident);
                    match &item.content {
                        Some((_, items)) => self.visit_items(items, &dir, &path)?,
                        _ => {
                            if item.attrs.iter().any(|a| a.path().is_ident("path")) {
                                bail!("the path attribute of module `{path}` is not supported");
                            }
                            files.push((dir, path));
                        }
                    }
                }
                Item::Enum(item) => {
                    if !derives_serde(&item.attrs)? {
                        continue;
                    }
                    let variants = item
                        .variants
                        .iter()
                        .map(|v| VariantDef {
                            name: v.ident.to_string(),
                            fields: fields(&v.fields),
                        })
                        .collect();
                    let def = EnumDef { variants };
                    insert(&mut self.types.lock().enums, path, &item.ident, def)?;
                }
                Item::Struct(item) => {
                    if !derives_serde(&item.attrs)? {
                        continue;
                    }
                    let def = StructDef {
                        fields: fields(&item.fields),
                    };
                    insert(&mut self.types.lock().structs, path, &item.ident, def)?;
                }
                Item::Macro(item) if item.mac.path.is_ident("bitflags") => {
                    let bitflags: BitFlags = item
                        .mac
                        .parse_body()
                        .context("could not parse a bitflags invocation")?;
                    if !derives_serde(&bitflags.attrs)? {
                        continue;
                    }
                    let def = StructDef {
                        fields: FieldsDef {
                            kind: FieldsKind::Tuple,
                            fields: vec![FieldDef {
                                name: None,
                                ty: tokens(&bitflags.repr),
                                attrs: vec![],
                            }],
                        },
                    };
                    insert(&mut self.types.lock().structs, path, &bitflags.name, def)?;
                }
                _ => {}
            }
        }
        files.par_iter().try_for_each(|(dir, path)| {
            let file = self.module_file(&dir, &path)?;
            self.visit_file(&file, &dir, &path)?;
            anyhow::Ok(())
        })?;
        Ok(())
    }

    fn module_file(&self, dir: &str, path: &str) -> Result<String> {
        let files = self.files.lock();
        for candidate in [format!("{dir}.rs"), format!("{dir}/mod.rs")] {
            if files.contains(&candidate) {
                return Ok(candidate);
            }
        }
        bail!("could not find the file of module `{path}`");
    }
}

fn insert<T>(defs: &mut BTreeMap<String, T>, path: &str, name: &Ident, def: T) -> Result<()> {
    let path = format!("{path}::{name}");
    if defs.insert(path.clone(), def).is_some() {
        bail!("`{path}` is defined more than once");
    }
    Ok(())
}

struct BitFlags {
    attrs: Vec<Attribute>,
    name: Ident,
    repr: Type,
}

impl Parse for BitFlags {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let _: Visibility = input.parse()?;
        let _: Token![struct] = input.parse()?;
        let name: Ident = input.parse()?;
        let repr;
        parenthesized!(repr in input);
        let _: Visibility = repr.parse()?;
        let repr: Type = repr.parse()?;
        let _: TokenStream = input.parse()?;
        Ok(Self { attrs, name, repr })
    }
}

fn fields(fields: &syn::Fields) -> FieldsDef {
    let kind = match fields {
        syn::Fields::Named(_) => FieldsKind::Struct,
        syn::Fields::Unnamed(_) => FieldsKind::Tuple,
        syn::Fields::Unit => FieldsKind::Unit,
    };
    let fields = fields
        .iter()
        .map(|f| FieldDef {
            name: f.ident.as_ref().map(|i| i.to_string()),
            ty: tokens(&f.ty),
            attrs: f
                .attrs
                .iter()
                .filter(|a| !a.path().is_ident("doc"))
                .map(tokens)
                .collect(),
        })
        .collect();
    FieldsDef { kind, fields }
}

fn derives_serde(attrs: &[Attribute]) -> Result<bool> {
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        let paths = attr
            .parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
            .context("could not parse a derive attribute")?;
        for path in paths {
            let Some(segment) = path.segments.last() else {
                continue;
            };
            if segment.ident == "Serialize" || segment.ident == "Deserialize" {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn tokens(tokens: &impl ToTokens) -> String {
    let string = tokens.to_token_stream().to_string();
    let mut res = String::new();
    let mut chars = string.chars().peekable();
    let mut prev = '\0';
    while let Some(c) = chars.next() {
        if c == ' ' {
            let next = chars.peek().copied().unwrap_or('\0');
            let squeeze = matches!(next, ',' | ';' | ':' | '<' | '>' | '(' | ')' | '[' | ']')
                || matches!(prev, ':' | '<' | '&' | '(' | '[' | '#' | '!');
            if squeeze {
                continue;
            }
        }
        res.push(c);
        prev = c;
    }
    res
}
