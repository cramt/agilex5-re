//! A small schema language for PDB v25 classes, so a class layout learnt
//! from the `PDB_ASCII_DUMP` trace is data, not code, and one interpreter
//! (`reader.rs`) decodes and re-emits the trace for every class.
//!
//! ```text
//! # comment
//! class DEV_PIN
//!   m_id            i32
//!   m_bool_data     bits
//!   m_int_data      arr 25 i32~        # `~`: read but not traced by Quartus
//!   m_string_data   arr 15 str
//!   m_pad_id_list   vec i32
//!   m_net           ptr DEV_PKG_NET
//! end
//! struct NAME ... end                  # an inline (non-pointer) object
//! ```
//!
//! Types (prefix notation): `i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 bool`,
//! `str`, `bits` (std::vector<bool>), `ptr CLASS`, `vec T`, `arr N T`,
//! `map K V`, `pair A B`, `obj STRUCT`. A trailing `~` on a scalar marks it
//! untraced.

use anyhow::{bail, ensure, Context, Result};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int { bytes: u8, signed: bool, traced: bool },
    F32 { traced: bool },
    F64 { traced: bool },
    Bool { traced: bool },
    Str,
    Bits,
    Ptr(String),
    Vec(Box<Ty>),
    Arr(usize, Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    Pair(Box<Ty>, Box<Ty>),
    Obj(String),
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Ty,
}

#[derive(Debug, Clone, Default)]
pub struct Schema {
    /// Pointer-addressable classes and inline structs share one namespace.
    pub types: HashMap<String, Vec<Field>>,
}

impl Schema {
    pub fn parse(src: &str) -> Result<Schema> {
        let mut s = Schema::default();
        let mut cur: Option<(String, Vec<Field>)> = None;
        for (ln, raw) in src.lines().enumerate() {
            let line = raw.split('#').next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            let ctx = || format!("schema line {}: {raw:?}", ln + 1);
            let mut words = line.split_whitespace();
            let first = words.next().unwrap();
            match (first, &mut cur) {
                ("class" | "struct", None) => {
                    let name = words.next().with_context(ctx)?.to_string();
                    cur = Some((name, Vec::new()));
                }
                ("end", Some(_)) => {
                    let (name, fields) = cur.take().unwrap();
                    ensure!(s.types.insert(name.clone(), fields).is_none(), "{name} defined twice");
                }
                (_, Some((_, fields))) => {
                    let toks: Vec<&str> = words.collect();
                    let mut it = toks.iter().copied();
                    let ty = parse_ty(&mut it).with_context(ctx)?;
                    ensure!(it.next().is_none(), "trailing words in {}", ctx());
                    fields.push(Field { name: first.to_string(), ty });
                }
                _ => bail!("unexpected {first:?} at {}", ctx()),
            }
        }
        ensure!(cur.is_none(), "schema ends inside a class");
        Ok(s)
    }

    pub fn fields(&self, class: &str) -> Result<&[Field]> {
        self.types.get(class).map(|v| v.as_slice()).with_context(|| format!("no schema for class {class}"))
    }
}

fn parse_ty<'a>(it: &mut impl Iterator<Item = &'a str>) -> Result<Ty> {
    let w = it.next().context("missing type")?;
    let (base, traced) = match w.strip_suffix('~') {
        Some(b) => (b, false),
        None => (w, true),
    };
    let int = |bytes, signed| Ok(Ty::Int { bytes, signed, traced });
    match base {
        "i8" => int(1, true),
        "i16" => int(2, true),
        "i32" => int(4, true),
        "i64" => int(8, true),
        "u8" => int(1, false),
        "u16" => int(2, false),
        "u32" => int(4, false),
        "u64" => int(8, false),
        "f32" => Ok(Ty::F32 { traced }),
        "f64" => Ok(Ty::F64 { traced }),
        "bool" => Ok(Ty::Bool { traced }),
        "str" => Ok(Ty::Str),
        "bits" => Ok(Ty::Bits),
        "ptr" => Ok(Ty::Ptr(it.next().context("ptr needs a class")?.to_string())),
        "obj" => Ok(Ty::Obj(it.next().context("obj needs a struct")?.to_string())),
        "vec" => Ok(Ty::Vec(Box::new(parse_ty(it)?))),
        "arr" => {
            let n: usize = it.next().context("arr needs a length")?.parse()?;
            Ok(Ty::Arr(n, Box::new(parse_ty(it)?)))
        }
        "map" => Ok(Ty::Map(Box::new(parse_ty(it)?), Box::new(parse_ty(it)?))),
        "pair" => Ok(Ty::Pair(Box::new(parse_ty(it)?), Box::new(parse_ty(it)?))),
        _ => bail!("unknown type {w:?}"),
    }
}
