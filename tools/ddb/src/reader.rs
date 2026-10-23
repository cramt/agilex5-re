//! Schema-driven decoder for PDB v25 bodies. It reads the body the way
//! `PDB_SEGMENT_READER` does (root object inline, every other object
//! deferred and drained table by table) and can re-emit the
//! `PDB_ASCII_DUMP` token stream (`verify/trace_tokens.py`), which is how a
//! schema is checked: byte-identical token streams and the exact end of the
//! body.
//!
//! Encoding rules, all observed against the 26.1.1 traces:
//! - Integers are stored raw, little-endian, no deltas (unlike Quartus 13).
//! - A pointer is one i32 `v`: 0 = null; `v & 3 == 0` = a new object of
//!   footer class `v >> 2`, taking the next id in its table; `v & 3 == 2` =
//!   a back-reference, id = last + (v >> 2) (arithmetic shift), where `last`
//!   is the table's previous back-reference (new objects don't move it).
//! - Deferred bodies are written table by table, last-created table first.

use anyhow::{bail, ensure, Context, Result};
use std::collections::HashMap;
use std::io::Write;

use crate::pdb::{Cursor, Footer};
use crate::schema::{Schema, Ty};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjRef {
    /// Footer class id of the object's actual class.
    pub class: u16,
    /// 1-based id in the pointer table of the declared type.
    pub id: u32,
}

#[derive(Debug, Clone)]
pub enum Val {
    I(i64),
    F(f64),
    B(bool),
    S(String),
    Bits(Vec<bool>),
    Ptr(Option<ObjRef>),
    List(Vec<Val>),
    Map(Vec<(Val, Val)>),
    Pair(Box<(Val, Val)>),
    /// Field values in schema order.
    Obj(Vec<Val>),
}

impl Val {
    pub fn int(&self) -> Option<i64> {
        match self {
            Val::I(v) => Some(*v),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            Val::S(s) => Some(s),
            _ => None,
        }
    }
    pub fn list(&self) -> &[Val] {
        match self {
            Val::List(v) => v,
            _ => &[],
        }
    }
    pub fn ptr(&self) -> Option<ObjRef> {
        match self {
            Val::Ptr(p) => *p,
            _ => None,
        }
    }
}

/// One decoded object with its class name, for field lookup by name.
pub struct Object<'s> {
    pub class: String,
    pub fields: &'s [crate::schema::Field],
    pub vals: Vec<Val>,
}

impl Object<'_> {
    pub fn get(&self, name: &str) -> Option<&Val> {
        self.fields.iter().position(|f| f.name == name).map(|i| &self.vals[i])
    }
}

#[derive(Default)]
struct Table {
    count: u32,
    last: u32,
    /// ids created but not yet serialized, with their actual class
    pending: Vec<(u32, u16)>,
}

pub struct Decoded<'s> {
    pub root: Object<'s>,
    /// (declared table name, id) -> object
    pub objects: HashMap<(String, u32), Object<'s>>,
    pub class_names: HashMap<u16, String>,
}

impl Decoded<'_> {
    pub fn deref(&self, table: &str, r: Option<ObjRef>) -> Option<&Object<'_>> {
        let r = r?;
        self.objects.get(&(table.to_string(), r.id))
    }
}

pub struct Reader<'a, 's> {
    c: Cursor<'a>,
    schema: &'s Schema,
    class_names: HashMap<u16, String>,
    class_ids: HashMap<String, u16>,
    tables: HashMap<String, Table>,
    /// Tables with pending objects, most recently created last.
    stack: Vec<String>,
    log: Option<&'a mut dyn Write>,
    /// Debug: which field is being read, for error messages.
    path: Vec<String>,
    /// DDB_POS=1: suffix every token with the body offset after it and the field path.
    show_pos: bool,
}

impl<'a, 's> Reader<'a, 's> {
    pub fn new(body: &'a [u8], footer: &Footer, schema: &'s Schema, log: Option<&'a mut dyn Write>) -> Self {
        let class_names: HashMap<u16, String> = footer.classes.iter().map(|c| (c.class_id as u16, c.name.clone())).collect();
        let class_ids = class_names.iter().map(|(k, v)| (v.clone(), *k)).collect();
        Reader { c: Cursor::new(body), schema, class_names, class_ids, tables: HashMap::new(), stack: Vec::new(), log, path: Vec::new(), show_pos: std::env::var_os("DDB_POS").is_some() }
    }

    fn emit(&mut self, f: impl FnOnce() -> String) -> Result<()> {
        if let Some(l) = self.log.as_deref_mut() {
            if self.show_pos {
                writeln!(l, "{} @{:x} {}", f(), self.c.pos(), self.path.join("."))?;
            } else {
                writeln!(l, "{}", f())?;
            }
        }
        Ok(())
    }

    fn where_(&self) -> String {
        format!("{} at body {:#x}", self.path.join("."), self.c.pos())
    }

    pub fn decode(mut self, root_type: &str) -> Result<Decoded<'s>> {
        let root_ptr = self.ptr(root_type)?.context("null root pointer")?;
        let root_class = self.class_names[&root_ptr.class].clone();
        // The root body follows inline instead of being deferred.
        let t = self.tables.get_mut(root_type).unwrap();
        t.pending.retain(|&(id, _)| id != root_ptr.id);
        if t.pending.is_empty() {
            self.stack.retain(|s| s != root_type);
        }
        let root = self.object(&root_class)?;
        let mut objects = HashMap::new();
        while let Some(table) = self.stack.pop() {
            let pending = std::mem::take(&mut self.tables.get_mut(&table).unwrap().pending);
            for (id, class) in pending {
                let cname = self.class_names[&class].clone();
                let o = self.object(&cname).with_context(|| format!("{table} #{id} ({cname})"))?;
                objects.insert((table.clone(), id), o);
            }
        }
        ensure!(self.c.is_empty(), "{} trailing bytes after the last object (body {:#x})", self.c.remaining(), self.c.pos());
        Ok(Decoded { root, objects, class_names: self.class_names })
    }

    fn object(&mut self, class: &str) -> Result<Object<'s>> {
        let fields = self.schema.fields(class)?;
        self.emit(|| format!("@{class}"))?;
        let mut vals = Vec::with_capacity(fields.len());
        for f in fields {
            self.path.push(format!("{class}::{}", f.name));
            vals.push(self.val(&f.ty)?);
            self.path.pop();
        }
        Ok(Object { class: class.to_string(), fields, vals })
    }

    fn count(&mut self) -> Result<usize> {
        let n = self.c.i32()?;
        self.emit(|| format!("H4 {n:08X}"))?;
        ensure!((0..1 << 28).contains(&n), "bad count {n} in {}", self.where_());
        Ok(n as usize)
    }

    fn val(&mut self, ty: &Ty) -> Result<Val> {
        Ok(match ty {
            &Ty::Int { bytes, signed, traced } => {
                let raw = self.c.bytes(bytes as usize).with_context(|| self.where_())?;
                let mut u = 0u64;
                for (i, b) in raw.iter().enumerate() {
                    u |= (*b as u64) << (8 * i);
                }
                if traced {
                    self.emit(|| format!("H{bytes} {u:0w$X}", w = 2 * bytes as usize))?;
                }
                let v = if signed {
                    let sh = 64 - 8 * bytes as u32;
                    ((u << sh) as i64) >> sh
                } else {
                    u as i64
                };
                Val::I(v)
            }
            &Ty::Bool { traced } => {
                let b = self.c.u8()?;
                if traced {
                    self.emit(|| format!("H1 {b:02X}"))?;
                }
                Val::B(b != 0)
            }
            &Ty::F32 { traced } => {
                let v = f32::from_le_bytes(self.c.bytes(4)?.try_into().unwrap()) as f64;
                if traced {
                    self.emit(|| format!("D {}", fmt_e(v)))?;
                }
                Val::F(v)
            }
            &Ty::F64 { traced } => {
                let v = f64::from_le_bytes(self.c.bytes(8)?.try_into().unwrap());
                if traced {
                    self.emit(|| format!("D {}", fmt_e(v)))?;
                }
                Val::F(v)
            }
            Ty::Str => {
                let n = self.count()?;
                let b = self.c.bytes(n).with_context(|| self.where_())?;
                let s = String::from_utf8_lossy(b).into_owned();
                self.emit(|| format!("S {}", latin1(b)))?;
                Val::S(s)
            }
            Ty::Bits => {
                let n = self.count()?;
                let mut out = Vec::with_capacity(n);
                for w in 0..n.div_ceil(32) {
                    let word = self.c.u32()?;
                    self.emit(|| format!("H4 {word:08X}"))?;
                    for b in 0..32.min(n - 32 * w) {
                        out.push(word >> b & 1 != 0);
                    }
                }
                Val::Bits(out)
            }
            Ty::Ptr(decl) => Val::Ptr(self.ptr(decl)?),
            Ty::Obj(s) => {
                let fields = self.schema.fields(s)?;
                let mut vals = Vec::with_capacity(fields.len());
                for f in fields {
                    self.path.push(format!("{s}::{}", f.name));
                    vals.push(self.val(&f.ty)?);
                    self.path.pop();
                }
                Val::Obj(vals)
            }
            Ty::Vec(t) => {
                let n = self.count()?;
                let mut v = Vec::with_capacity(n.min(1 << 20));
                for i in 0..n {
                    self.path.push(format!("[{i}]"));
                    v.push(self.val(t)?);
                    self.path.pop();
                }
                Val::List(v)
            }
            Ty::Arr(n, t) => {
                let mut v = Vec::with_capacity(*n);
                for i in 0..*n {
                    self.path.push(format!("[{i}]"));
                    v.push(self.val(t)?);
                    self.path.pop();
                }
                Val::List(v)
            }
            Ty::Map(k, t) => {
                let n = self.count()?;
                let mut v = Vec::with_capacity(n.min(1 << 20));
                for i in 0..n {
                    self.path.push(format!("{{{i}}}"));
                    let kk = self.val(k)?;
                    let vv = self.val(t)?;
                    self.path.pop();
                    v.push((kk, vv));
                }
                Val::Map(v)
            }
            Ty::Pair(a, b) => {
                let a = self.val(a)?;
                let b = self.val(b)?;
                Val::Pair(Box::new((a, b)))
            }
        })
    }

    fn ptr(&mut self, decl: &str) -> Result<Option<ObjRef>> {
        let v = self.c.i32()?;
        let r = match v & 3 {
            _ if v == 0 => None,
            0 => {
                let class = (v >> 2) as u16;
                let cname = self.class_names.get(&class).with_context(|| format!("new object of unknown class {class} in {}", self.where_()))?;
                let _ = cname;
                let t = self.tables.entry(decl.to_string()).or_default();
                t.count += 1;
                let id = t.count;
                t.pending.push((id, class));
                if !self.stack.iter().any(|s| s == decl) {
                    self.stack.push(decl.to_string());
                }
                Some(ObjRef { class, id })
            }
            2 => {
                let at = self.where_();
                let t = self.tables.entry(decl.to_string()).or_default();
                let id = (t.last as i64 + (v >> 2) as i64) as u32;
                ensure!(id >= 1 && id <= t.count, "back-reference {v} -> id {id} outside {decl} table of {} in {at}", t.count);
                t.last = id;
                // The actual class of a back-referenced object is whatever
                // created it; the caller resolves it through the table.
                Some(ObjRef { class: 0, id })
            }
            _ => bail!("pointer value {v:#x} with tag {} in {}", v & 3, self.where_()),
        };
        let tag = match r {
            None => String::new(),
            Some(r) => format!(" {decl}#{}", r.id),
        };
        self.emit(|| format!("P {v:+}{tag}"))?;
        Ok(r)
    }

    pub fn class_id(&self, name: &str) -> Option<u16> {
        self.class_ids.get(name).copied()
    }
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// C's `%.15E`: mantissa with 15 decimals, exponent signed, at least 2 digits.
pub fn fmt_e(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let s = format!("{v:.15E}");
    let (m, e) = s.split_once('E').unwrap();
    let e: i32 = e.parse().unwrap();
    format!("{m}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}
