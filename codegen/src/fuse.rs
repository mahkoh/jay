use crate::format_rust;
use crate::get_absolute_path;
use crate::phf_generator;
use crate::phf_generator::HashState;
use crate::tokens::Symbol;
use crate::tokens::Token;
use crate::tokens::TokenKind;
use crate::tokens::TreeDelim;
use crate::tokens::tokenize;
use crate::update;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use isnt::std_1::collections::IsntHashSetExt;
use rayon::prelude::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

const GENERATED_START: &str = "// FUSE GENERATED START\n";
const GENERATED_STOP: &str = "// FUSE GENERATED STOP\n";

pub fn main() -> Result<()> {
    let root = get_absolute_path("src/");
    let root = root.to_str().unwrap();
    let mut files = vec![];
    for file in WalkDir::new(root) {
        let file = file?;
        let path = file.path().to_str().unwrap();
        if !path.ends_with("_g_fuse.rs") {
            continue;
        }
        files.push(file.into_path());
    }
    files.sort();
    let mut parsed = files
        .par_iter()
        .map(|path| {
            parse_file(root, path).with_context(|| anyhow!("Parsing file {}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let globals: Vec<_> = parsed
        .iter_mut()
        .flat_map(|p| p.ext.parsed.dirs.extract_if(.., |dir| dir.global))
        .collect();
    let resolved_globals = resolve_inherits(&globals, &HashMap::default())?;
    parsed.push(ParsedExt2 {
        error_path: "globals".to_string().into(),
        ext: ParsedExt {
            relative_path: "utils/fuse/fuse_globals.rs".to_string(),
            module: "globals".to_string(),
            out_path: "fuse/globals.rs".to_string(),
            parsed: Parsed { dirs: globals },
            is_global: true,
        },
    });
    let targets = parsed
        .into_par_iter()
        .map(|parsed| {
            handle_file(parsed.ext, &resolved_globals)
                .with_context(|| anyhow!("Handling file {}", parsed.error_path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let build_rs = generate_build_rs(&targets)?;
    update("build/fuse/generated.rs", &build_rs)?;
    let module_names: HashSet<_> = targets
        .iter()
        .map(|t| format!("m_{}.rs", t.module))
        .collect();
    for file in std::fs::read_dir(get_absolute_path("build/fuse/generated"))? {
        let file = file?;
        let file_name = file.file_name();
        let file_name = file_name.to_str().unwrap();
        if module_names.not_contains(file_name) {
            std::fs::remove_file(file.path())?;
        }
    }
    Ok(())
}

fn parse_file(root: &str, absolute_path: &Path) -> Result<ParsedExt2> {
    let old = std::fs::read_to_string(absolute_path).context("Reading")?;
    let Some(desc) = old.strip_prefix("/*") else {
        bail!("File does not start with /*");
    };
    let Some((desc, _)) = desc.split_once("*/") else {
        bail!("File does not contain */");
    };
    let parsed = parse(desc.as_bytes()).context("Parsing")?;
    let Some(relative_path) = absolute_path.to_str().unwrap().strip_prefix(root) else {
        bail!("{} does not start with {root}", absolute_path.display());
    };
    let module = blake3::hash(relative_path.as_bytes()).to_hex().to_string();
    let out_path = format!("fuse/m_{module}.rs");
    let generated = generate_include(&out_path)?;
    let generated = format_rust(absolute_path, &generated)?;
    let new = if let Some((lo, hi)) = old.split_once(GENERATED_START) {
        let Some((_, hi)) = hi.split_once(GENERATED_STOP) else {
            bail!("File contains start marker but not stop marker");
        };
        format!("{lo}{GENERATED_START}{generated}{GENERATED_STOP}{hi}")
    } else {
        format!("{old}\n{GENERATED_START}{generated}{GENERATED_STOP}")
    };
    if old != new {
        std::fs::write(absolute_path, new)?;
    }
    Ok(ParsedExt2 {
        error_path: absolute_path.to_path_buf(),
        ext: ParsedExt {
            relative_path: relative_path.to_string(),
            module,
            parsed,
            out_path,
            is_global: false,
        },
    })
}

fn handle_file(parsed: ParsedExt, globals: &HashMap<String, Vec<Dirent>>) -> Result<Target> {
    let ParsedExt {
        relative_path,
        module,
        parsed,
        out_path,
        is_global,
    } = parsed;
    let resolved_cache;
    let resolved = if is_global {
        globals
    } else {
        resolved_cache = resolve_inherits(&parsed.dirs, globals)?;
        &resolved_cache
    };
    let dirs = parsed
        .dirs
        .iter()
        .map(|dir| {
            let dirents = resolved.get(dir.name.as_str()).unwrap();
            PhfDir {
                phf: compute_phf(dir, dirents),
                name: dir.name.clone(),
                abstract_: dir.abstract_,
                dirents: dirents.clone(),
                parents: compute_parents(dir),
            }
        })
        .collect();
    let target = Target {
        relative_path,
        module,
        out_path,
        dirs,
        is_global,
    };
    let module = generate_build_rs_module(&target)?;
    let module_path = format!("build/fuse/generated/m_{}.rs", target.module);
    update(&module_path, &module)?;
    Ok(target)
}

fn parse(data: &[u8]) -> Result<Parsed> {
    let tokens = tokenize(data)?;
    let mut parser = Parser {
        pos: 0,
        tokens: &tokens,
    };
    parser.parse()
}

struct Parser<'a> {
    pos: usize,
    tokens: &'a [Token<'a>],
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> Result<Parsed> {
        let mut res = Parsed {
            dirs: Default::default(),
        };
        while !self.eof() {
            let (line, ty) = self.expect_ident()?;
            match ty {
                "dir" => res.dirs.push(self.parse_dir()?),
                _ => bail!("In line {}: Unexpected entry {:?}", line, ty),
            }
        }
        Ok(res)
    }

    fn eof(&self) -> bool {
        self.pos == self.tokens.len()
    }

    fn not_eof(&self) -> Result<()> {
        if self.eof() {
            bail!("Unexpected eof");
        }
        Ok(())
    }

    fn expect_ident(&mut self) -> Result<(u32, &'a str)> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Ident(id) => Ok((token.line, *id)),
            k => bail!(
                "In line {}: Expected identifier, found {}",
                token.line,
                k.name()
            ),
        }
    }

    fn expect_num(&mut self) -> Result<(u32, u32)> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Num(num) => Ok((token.line, *num)),
            k => bail!(
                "In line {}: Expected number, found {}",
                token.line,
                k.name()
            ),
        }
    }

    fn parse_dir(&mut self) -> Result<Dir> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            let mut abstract_ = false;
            let mut global = false;
            self.parse_attribs(|_parser, line, name| {
                match name {
                    "abstract" => abstract_ = true,
                    "global" => global = true,
                    _ => bail!("In line {}: Unexpected entry {:?}", line, name),
                }
                Ok(())
            })?;
            let (_, tokens) = self.expect_tree(TreeDelim::Brace)?;
            let mut parser = Parser { pos: 0, tokens };
            let mut dirents = vec![];
            let mut meta = vec![];
            while !parser.eof() {
                let token = &parser.tokens[parser.pos];
                match token.kind {
                    TokenKind::Ident(_) => {
                        dirents.push(parser.parse_dirent()?);
                    }
                    TokenKind::Symbol(_) => {
                        meta.push(parser.parse_meta_dirent()?);
                    }
                    _ => bail!("In line {}: Unexpected token {:?}", token.line, token.kind),
                }
                if !parser.eof() {
                    parser.expect_symbol(Symbol::Comma)?;
                }
            }
            Ok(Dir {
                name: name.to_string(),
                abstract_,
                global,
                dirents,
                meta,
            })
        })();
        res.with_context(|| format!("While parsing dir starting at line {}", line))
    }

    fn parse_dirent(&mut self) -> Result<Dirent> {
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| {
            self.expect_symbol(Symbol::Colon)?;
            let ty = self.parse_type()?;
            let mut opt = false;
            let mut other = false;
            let mut no_timeout = false;
            let mut key = None;
            self.parse_attribs(|parser, line, name| {
                match name {
                    "opt" => opt = true,
                    "other" => other = true,
                    "no_timeout" => no_timeout = true,
                    "key" => {
                        parser.expect_symbol(Symbol::Equals)?;
                        let (_, num) = parser.expect_num()?;
                        key = Some(num);
                    }
                    _ => bail!("In line {}: Unexpected entry {:?}", line, name),
                }
                Ok(())
            })?;
            Ok(Dirent {
                name: name.to_owned(),
                ty,
                opt,
                other,
                no_timeout,
                inherited: false,
                key,
            })
        })();
        res.with_context(|| format!("While parsing dirent starting at line {}", line))
    }

    fn parse_attribs(
        &mut self,
        mut handle_attrib: impl FnMut(&mut Parser, u32, &str) -> Result<()>,
    ) -> Result<()> {
        if !self.eof()
            && let TokenKind::Tree {
                delim: TreeDelim::Paren,
                ..
            } = self.tokens[self.pos].kind
        {
            let (_, body) = self.expect_tree(TreeDelim::Paren)?;
            let mut parser = Parser {
                pos: 0,
                tokens: body,
            };
            while !parser.eof() {
                let (line, name) = parser.expect_ident()?;
                handle_attrib(&mut parser, line, name)?;
                if !parser.eof() {
                    parser.expect_symbol(Symbol::Comma)?;
                }
            }
        }
        Ok(())
    }

    fn parse_meta_dirent(&mut self) -> Result<MetaDirent> {
        self.expect_symbol(Symbol::At)?;
        let (line, name) = self.expect_ident()?;
        let res: Result<_> = (|| match name {
            "inherit" => {
                let (_, name) = self.expect_ident()?;
                Ok(MetaDirent::Inherit {
                    name: name.to_string(),
                })
            }
            _ => bail!("Unexpected instruction {name}"),
        })();
        res.with_context(|| format!("While parsing instruction starting at line {}", line))
    }

    fn expect_symbol(&mut self, symbol: Symbol) -> Result<()> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Symbol(s) if *s == symbol => Ok(()),
            k => bail!(
                "In line {}: Expected {}, found {}",
                token.line,
                symbol.name(),
                k.name()
            ),
        }
    }

    fn expect_tree_(&mut self) -> Result<(u32, TreeDelim, &'a [Token<'a>])> {
        self.not_eof()?;
        let token = &self.tokens[self.pos];
        self.pos += 1;
        match &token.kind {
            TokenKind::Tree { delim, body } => Ok((token.line, *delim, body)),
            k => bail!("In line {}: Expected tree, found {}", token.line, k.name()),
        }
    }

    fn expect_tree(&mut self, exp_delim: TreeDelim) -> Result<(u32, &'a [Token<'a>])> {
        let (line, delim, tokens) = self.expect_tree_()?;
        if delim == exp_delim {
            Ok((line, tokens))
        } else {
            bail!(
                "In line {}: Expected {:?}-delimited tree, found {:?}-delimited tree",
                line,
                exp_delim,
                delim.opening()
            )
        }
    }

    fn parse_type(&mut self) -> Result<DirentTy> {
        self.not_eof()?;
        let (_, ty) = self.expect_ident()?;
        let ty = match ty.as_bytes() {
            b"reg" => DirentTy::Reg,
            b"view" => DirentTy::View,
            b"custom" => DirentTy::Custom,
            b"link" => DirentTy::Link,
            _ => bail!("Unknown type {}", ty),
        };
        Ok(ty)
    }
}

struct Target {
    relative_path: String,
    out_path: String,
    is_global: bool,
    module: String,
    dirs: Vec<PhfDir>,
}

struct Parsed {
    dirs: Vec<Dir>,
}

struct ParsedExt {
    relative_path: String,
    module: String,
    parsed: Parsed,
    out_path: String,
    is_global: bool,
}

struct ParsedExt2 {
    error_path: PathBuf,
    ext: ParsedExt,
}

#[derive(Debug)]
struct Dir {
    name: String,
    abstract_: bool,
    global: bool,
    dirents: Vec<Dirent>,
    meta: Vec<MetaDirent>,
}

struct PhfDir {
    name: String,
    abstract_: bool,
    dirents: Vec<Dirent>,
    phf: HashState,
    parents: Vec<String>,
}

#[derive(Clone, Debug)]
struct Dirent {
    name: String,
    ty: DirentTy,
    opt: bool,
    other: bool,
    no_timeout: bool,
    inherited: bool,
    key: Option<u32>,
}

#[derive(Debug)]
enum MetaDirent {
    Inherit { name: String },
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum DirentTy {
    Reg,
    View,
    Custom,
    Link,
}

fn resolve_inherits(
    dirs: &[Dir],
    globals: &HashMap<String, Vec<Dirent>>,
) -> Result<HashMap<String, Vec<Dirent>>> {
    let mut dirs_by_name = HashMap::new();
    for dir in dirs {
        if dirs_by_name.insert(dir.name.as_str(), dir).is_some() {
            bail!("Duplicate dir name {}", dir.name);
        }
        if globals.contains_key(&dir.name) {
            bail!("Dir name {} conflicts with global name", dir.name);
        }
    }
    let mut resolved = HashMap::new();
    let mut pending = HashSet::new();
    for dir in dirs {
        resolve_inherit(dir, &dirs_by_name, &mut resolved, &mut pending, globals)?;
    }
    for (k, v) in &resolved {
        let mut set = HashSet::new();
        for v in v {
            if !set.insert(v.name.as_str()) {
                bail!("Directory {k} contains duplicate name {}", v.name);
            }
        }
    }
    Ok(resolved
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect())
}

fn resolve_inherit<'a>(
    dir: &'a Dir,
    dirs: &HashMap<&'a str, &'a Dir>,
    resolved: &mut HashMap<&'a str, Vec<Dirent>>,
    pending: &mut HashSet<&'a str>,
    globals: &HashMap<String, Vec<Dirent>>,
) -> Result<()> {
    if resolved.contains_key(&*dir.name) {
        return Ok(());
    }
    if !pending.insert(&dir.name) {
        bail!("Recursion while resolving dirents of {}", dir.name);
    }
    let mut dirents: Vec<_> = dir.dirents.to_vec();
    for meta in &dir.meta {
        match meta {
            MetaDirent::Inherit { name } => {
                let ents = match resolved.entry(name.as_str()) {
                    Entry::Occupied(e) => e.into_mut(),
                    Entry::Vacant(_) => match globals.get(name.as_str()) {
                        Some(e) => e,
                        None => {
                            let Some(dir) = dirs.get(name.as_str()) else {
                                bail!("Parent dirent {} of {} does not exist", name, dir.name);
                            };
                            resolve_inherit(dir, dirs, resolved, pending, globals)?;
                            resolved.get(name.as_str()).unwrap()
                        }
                    },
                };
                dirents.extend(ents.iter().cloned().map(|mut ent| {
                    ent.inherited = true;
                    ent
                }));
            }
        }
    }
    resolved.insert(&dir.name, dirents);
    Ok(())
}

fn compute_phf(dir: &Dir, dirents: &[Dirent]) -> HashState {
    if dir.abstract_ || dirents.is_empty() {
        return HashState::default();
    }
    let keys: Vec<_> = dirents.iter().map(|v| v.name.as_str()).collect();
    phf_generator::generate_hash(&keys)
}

fn compute_parents(dir: &Dir) -> Vec<String> {
    let mut parents = vec![];
    for meta in &dir.meta {
        match meta {
            MetaDirent::Inherit { name } => parents.push(name.clone()),
        }
    }
    parents
}

fn generate_include(module: &str) -> Result<String> {
    let mut out = String::new();
    define_w!(out);
    wl!(r#"include!(concat!(env!("OUT_DIR"), "/{module}",));"#);
    Ok(out)
}

fn generate_build_rs(targets: &[Target]) -> Result<String> {
    let mut out = String::new();
    define_w!(out);
    wl!("use super::*;");
    wl!();
    for target in targets {
        wl!("mod m_{};", &target.module);
    }
    wl!();
    wl!("pub static TARGETS: &[&Target] = &[");
    for target in targets {
        wl!("    &m_{}::TARGET, //", target.module);
    }
    wl!("];");
    Ok(out)
}

fn generate_build_rs_module(target: &Target) -> Result<String> {
    let mut out = String::new();
    define_w!(out);
    wl!("// {}", target.relative_path);
    wl!();
    wl!("use super::*;");
    wl!();
    wl!("pub static TARGET: Target = Target {{");
    wl!("    path: {:?},", target.out_path);
    wl!("    is_global: {},", target.is_global);
    wl!("    dirs: &[ //");
    for dir in &target.dirs {
        wl!("        Dir {{");
        wl!("            name: {:?},", dir.name);
        wl!("            abstract_: {},", dir.abstract_);
        wl!("            parents: &{:?},", dir.parents);
        wl!("            dirents: &[ //");
        for ent in &dir.dirents {
            let ty = match ent.ty {
                DirentTy::Reg => "Reg",
                DirentTy::View => "View",
                DirentTy::Custom => "Custom",
                DirentTy::Link => "Link",
            };
            wl!("                Ent {{");
            wl!("                    name: {:?},", ent.name);
            wl!("                    camel: {:?},", to_camel(&ent.name));
            wl!("                    ty: EntTy::{ty},");
            wl!("                    opt: {},", ent.opt);
            wl!("                    other: {},", ent.other);
            wl!("                    no_timeout: {},", ent.no_timeout);
            wl!("                    inherited: {},", ent.inherited);
            wl!("                    predefined_key: {:?},", ent.key);
            wl!("                }},");
        }
        wl!("            ],");
        wl!("            phf: PhfMap {{");
        wl!("                key: {},", dir.phf.key);
        wl!("                disps: &{:?},", dir.phf.disps);
        wl!("                map: &{:?},", dir.phf.map);
        wl!("            }},");
        wl!("        }},");
    }
    wl!("    ],");
    wl!("}};");
    Ok(out)
}

fn to_camel(s: &str) -> String {
    let mut last_was_underscore = true;
    let mut res = String::new();
    for mut b in s.as_bytes().iter().copied() {
        if b == b'_' {
            last_was_underscore = true;
        } else {
            if last_was_underscore {
                b = b.to_ascii_uppercase()
            }
            res.push(b as char);
            last_was_underscore = false;
        }
    }
    res
}
