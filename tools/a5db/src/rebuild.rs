//! A design as a toolchain would hand it to a5db, read back from an image,
//! and the image rebuilt from it (README (q)).
//!
//! A design file is lines of
//!
//!   pin    <ball>=<use>[:<std>]          I/O, through db/sm4revb/io/template.tsv
//!   edge   <src node> <dst node>         a routing graph edge
//!   attr   <uid> <attribute> <setting>   a CUSTOM attribute
//!   direct <uid> <attribute> <bits>      a DIRECT attribute, MSB first
//!   bit    <uid> <bit> <0|1>             a named bit no attribute explains
//!   raw    <object/part> <offset> <0|1>  an image bit no named bit covers
//!
//! over the idle image, written in that order. `extract` writes the lines
//! that turn the idle image into a given one, preferring the higher kinds:
//! the pins it is told about, every routing edge the image selects, then per
//! instance the attributes whose setting differs; `bit` and `raw` lines are
//! what the vocabulary above doesn't cover.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};

use crate::db::{BitPos, Db, PARTS};
use crate::graph::{Graph, Join};
use crate::{blank, pins, route};

/// Covered (named) image bits, as a per-byte mask.
fn named_mask(db: &Db, len: usize) -> Result<Vec<u8>> {
    let mut cov = vec![0u8; len];
    for t in db.types()? {
        let map = db.map(&t);
        for u in map.uids() {
            for (_, p) in map.bits(u) {
                cov[p.byte] |= 1 << p.bit;
            }
        }
    }
    Ok(cov)
}

/// One instance's attr, direct and bit lines: the CUSTOM settings that
/// explain its differing bits best, then DIRECT values, then single bits.
#[allow(clippy::too_many_arguments)]
fn extract_instance(db: &Db, t: &str, u: &str, bits: &[(&str, BitPos)], q: &[u8], r: &mut [u8], text: &mut String, c: &mut [usize; 3]) -> Result<()> {
    let pos: HashMap<&str, BitPos> = bits.iter().map(|(n, p)| (*n, *p)).collect();
    for a in db.bcm.attributes(t) {
        let settings = db.bcm.settings(&a);
        if settings.is_empty() {
            continue;
        }
        let full = |b: &str| format!("{}{b}", a.prefix);
        // Attribute bits that differ; skip the attribute if none do.
        let mine: Vec<BitPos> = settings[0].bits.iter().filter_map(|b| pos.get(full(b).as_str()).copied()).collect();
        if mine.len() != settings[0].bits.len() || mine.iter().all(|p| p.get(q) == p.get(r)) {
            continue;
        }
        // The setting whose written bits match the image best, all cared-for bits equal.
        let best = settings
            .iter()
            .filter(|s| s.bits.iter().zip(&s.values).all(|(b, &(v, cares))| !cares || pos[full(b).as_str()].get(q) == v))
            .max_by_key(|s| s.bits.iter().zip(&s.values).filter(|(b, &(v, _))| pos[full(b).as_str()].get(q) == v).count());
        if let Some(s) = best {
            for (b, &(v, _)) in s.bits.iter().zip(&s.values) {
                pos[full(b).as_str()].set(r, v);
            }
            writeln!(text, "attr\t{u}\t{}\t{}", a.path, s.name)?;
            c[0] += 1;
        }
    }
    for d in direct_attrs(db, t) {
        let names = db.bcm.direct_bits(t, &d).unwrap();
        let ps: Option<Vec<BitPos>> = names.iter().map(|n| pos.get(n.as_str()).copied()).collect();
        let Some(ps) = ps else { continue };
        if ps.iter().all(|p| p.get(q) == p.get(r)) {
            continue;
        }
        let v: String = ps.iter().map(|p| if p.get(q) { '1' } else { '0' }).collect();
        for p in &ps {
            p.set(r, p.get(q));
        }
        writeln!(text, "direct\t{u}\t{d}\t{v}")?;
        c[1] += 1;
    }
    for (n, p) in bits {
        if p.get(q) != p.get(r) {
            p.set(r, p.get(q));
            writeln!(text, "bit\t{u}\t{n}\t{}", p.get(q) as u8)?;
            c[2] += 1;
        }
    }
    Ok(())
}

fn direct_attrs(db: &Db, btype: &str) -> Vec<String> {
    let text = std::fs::read_to_string(db.dir.join("direct.tsv")).unwrap_or_default();
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let mut f = l.split('\t');
            (f.next()? == btype).then(|| f.next().map(str::to_owned)).flatten()
        })
        .collect()
}

/// Write a CUSTOM setting's bits (every bit the setting lists, as `a5db set`).
fn write_setting(db: &mut Db, btype: &str, uid: &str, path: &str, setting: &str, img: &mut [u8]) -> Result<()> {
    let a = db.attribute(btype, path).with_context(|| format!("{btype} has no attribute {path}"))?;
    let s = db.bcm.settings(&a).iter().find(|s| s.name == setting).with_context(|| format!("{uid} {path} has no setting {setting}"))?;
    let map = db.map(btype);
    for (b, &(v, _)) in s.bits.iter().zip(&s.values) {
        map.pos(uid, &format!("{}{b}", a.prefix)).with_context(|| format!("{uid} {}{b} is not mapped", a.prefix))?.set(img, v);
    }
    Ok(())
}

fn write_direct(db: &Db, btype: &str, uid: &str, path: &str, value: &str, img: &mut [u8]) -> Result<()> {
    let bits = db.bcm.direct_bits(btype, path).with_context(|| format!("{btype} {path} is not DIRECT"))?;
    if bits.len() != value.len() {
        bail!("{uid} {path}: {} bits, got {}", bits.len(), value.len());
    }
    let map = db.map(btype);
    for (b, c) in bits.iter().zip(value.bytes()) {
        map.pos(uid, b).with_context(|| format!("{uid} {b} is not mapped"))?.set(img, c == b'1');
    }
    Ok(())
}

fn write_raw(part: &str, off: usize, v: bool, img: &mut [u8]) -> Result<()> {
    let (o, p) = part.split_once('/').with_context(|| format!("{part}: expected object/part"))?;
    let (_, _, start, _) = PARTS.iter().find(|(a, b, ..)| *a == o && *b == p).with_context(|| format!("no part {part}"))?;
    let m = 1u8 << (off % 8);
    img[start + off / 8] = if v { img[start + off / 8] | m } else { img[start + off / 8] & !m };
    Ok(())
}

/// The edges' bits, written into img.
fn write_edges(join: &mut Join, edges: &[usize], img: &mut [u8]) -> Result<()> {
    for &e in edges {
        if let Some(Ok(w)) = join.edge(e)? {
            for (p, v) in w {
                p.set(img, v);
            }
        }
    }
    Ok(())
}

pub fn extract(db: &mut Db, graph: &Graph, q: &[u8], pin_specs: &[String]) -> Result<String> {
    let t0 = std::time::Instant::now();
    let idle = blank::image(db)?;
    let mut r = idle.clone();
    let mut text = String::from("# a5db design (README (q))\n");
    let pins = pins::parse(db, pin_specs)?;
    for (u, b, v) in pins::table(db, &pins)? {
        crate::write_rows(db, &mut r, &[(u, b, v)])?;
    }
    for s in pin_specs {
        writeln!(text, "pin\t{s}")?;
    }
    // Only instances that differ from the idle image can hold a decoded
    // edge or attribute; the rest are skipped without touching their bits.
    let dirty: Vec<String> = blank::dirty(db, &idle, q)?.into_iter().map(str::to_owned).collect();
    let dirty_set: rustc_hash::FxHashSet<&str> = dirty.iter().map(String::as_str).collect();
    eprintln!("extract: {} instances differ from idle ({:.1} s)", dirty.len(), t0.elapsed().as_secs_f64());
    let mut join = Join::new(db, graph);
    let edges = route::decode_in(&mut join, graph, q, Some(&dirty_set))?;
    write_edges(&mut join, &edges, &mut r)?;
    drop(join);
    eprintln!("extract: {} edges decoded ({:.1} s)", edges.len(), t0.elapsed().as_secs_f64());
    for &e in &edges {
        writeln!(text, "edge\t{}\t{}", graph.names[route::edge_src(graph, e) as usize], graph.names[graph.dst[e] as usize])?;
    }
    let covered = named_mask(db, q.len())?;
    let mut counts: BTreeMap<String, [usize; 3]> = BTreeMap::new();
    for t in db.types()? {
        let map = db.map(&t);
        for u in map.uids() {
            if !map.bits(u).any(|(_, p)| p.get(q) != p.get(&r)) {
                continue;
            }
            let bits: Vec<(&str, BitPos)> = map.bits(u).collect();
            extract_instance(db, &t, u, &bits, q, &mut r, &mut text, counts.entry(t.clone()).or_default())?;
        }
    }
    let mut raws = 0;
    for (o, pt, start, n) in PARTS {
        for i in *start..start + n {
            if q[i] == r[i] {
                continue;
            }
            for bit in 0..8u8 {
                if (q[i] ^ r[i]) >> bit & 1 == 1 && covered[i] >> bit & 1 == 0 {
                    writeln!(text, "raw\t{o}/{pt}\t{}\t{}", (i - start) * 8 + bit as usize, q[i] >> bit & 1)?;
                    raws += 1;
                }
            }
        }
    }
    eprintln!("{} pins, {} edges, {raws} raw bits; per type (attr, direct, bit):", pin_specs.len(), edges.len());
    for (t, c) in &counts {
        eprintln!("  {t}\t{}\t{}\t{}", c[0], c[1], c[2]);
    }
    Ok(text)
}

pub fn build(db: &mut Db, graph: &Graph, design: &str) -> Result<Vec<u8>> {
    let t = std::time::Instant::now();
    let mut img = blank::image(db)?;
    eprintln!("build: idle image {:.1} s", t.elapsed().as_secs_f64());
    let lines: Vec<Vec<&str>> = design.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).map(|l| l.split('\t').collect()).collect();
    let specs: Vec<String> = lines.iter().filter(|f| f[0] == "pin").map(|f| f[1].to_owned()).collect();
    let pins = pins::parse(db, &specs)?;
    let rows = pins::table(db, &pins)?;
    crate::write_rows(db, &mut img, &rows)?;
    let idx = graph.lookup(lines.iter().filter(|f| f[0] == "edge").flat_map(|f| [f[1], f[2]]));
    eprintln!("build: pins and graph index {:.1} s", t.elapsed().as_secs_f64());
    let mut edges = Vec::new();
    for f in lines.iter().filter(|f| f[0] == "edge") {
        let (s, d) = (idx.get(f[1]).with_context(|| format!("no node {}", f[1]))?, idx.get(f[2]).with_context(|| format!("no node {}", f[2]))?);
        edges.push(graph.edge(*s, *d).with_context(|| format!("no edge {} -> {}", f[1], f[2]))?);
    }
    let mut join = Join::new(db, graph);
    write_edges(&mut join, &edges, &mut img)?;
    drop(join);
    eprintln!("build: edges {:.1} s", t.elapsed().as_secs_f64());
    for kind in ["attr", "direct", "bit", "raw"] {
        for f in lines.iter().filter(|f| f[0] == kind) {
            match kind {
                "attr" => {
                    let t = db.type_of(f[1])?;
                    write_setting(db, &t, f[1], f[2], f[3], &mut img)?
                }
                "direct" => {
                    let t = db.type_of(f[1])?;
                    write_direct(db, &t, f[1], f[2], f[3], &mut img)?
                }
                "bit" => {
                    let t = db.type_of(f[1])?;
                    db.map(&t).pos(f[1], f[2]).with_context(|| format!("{} {} is not mapped", f[1], f[2]))?.set(&mut img, f[3] == "1")
                }
                _ => write_raw(f[1], f[2].parse()?, f[3] == "1", &mut img)?,
            }
        }
    }
    Ok(img)
}
