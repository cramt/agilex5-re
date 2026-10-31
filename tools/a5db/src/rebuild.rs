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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};

use crate::db::{BitPos, Db, PARTS};
use crate::graph::{Graph, Join};
use crate::{blank, pins, route};

/// Every named bit of every instance, by type, with its position.
fn named(db: &mut Db) -> Result<Vec<(String, String, Vec<(String, BitPos)>)>> {
    let mut out = Vec::new();
    for t in db.types()? {
        db.load(&t)?;
        let map = db.map(&t);
        for u in map.uids() {
            let bits = map.bit_names(u).into_iter().map(|b| (b.to_owned(), map.pos(u, b).unwrap())).collect();
            out.push((t.clone(), u.to_owned(), bits));
        }
    }
    Ok(out)
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
fn write_setting(db: &Db, btype: &str, uid: &str, path: &str, setting: &str, img: &mut [u8]) -> Result<()> {
    let a = db.bcm.attributes(btype).into_iter().find(|a| a.path == path).with_context(|| format!("{btype} has no attribute {path}"))?;
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
    let mut r = blank::image(db)?;
    let mut text = String::from("# a5db design (README (q))\n");
    let pins = pins::parse(db, pin_specs)?;
    for (u, b, v) in pins::table(db, &pins)? {
        crate::write_rows(db, &mut r, &[(u, b, v)])?;
    }
    for s in pin_specs {
        writeln!(text, "pin\t{s}")?;
    }
    let mut join = Join::new(db, graph);
    let edges = route::decode(&mut join, graph, q)?;
    write_edges(&mut join, &edges, &mut r)?;
    drop(join);
    let mut src = vec![0u32; graph.dst.len()];
    for s in 0..graph.names.len() as u32 {
        for e in graph.fanout(s) {
            src[e] = s;
        }
    }
    for &e in &edges {
        writeln!(text, "edge\t{}\t{}", graph.names[src[e] as usize], graph.names[graph.dst[e] as usize])?;
    }
    let all = named(db)?;
    let mut covered: HashSet<(usize, u8)> = HashSet::new();
    let mut counts: BTreeMap<String, [usize; 3]> = BTreeMap::new();
    for (t, u, bits) in &all {
        for (_, p) in bits {
            covered.insert((p.byte, p.bit));
        }
        if !bits.iter().any(|(_, p)| p.get(q) != p.get(&r)) {
            continue;
        }
        let c = counts.entry(t.clone()).or_default();
        let pos: HashMap<&str, BitPos> = bits.iter().map(|(n, p)| (n.as_str(), *p)).collect();
        for a in db.bcm.attributes(t) {
            let settings = db.bcm.settings(&a);
            if settings.is_empty() {
                continue;
            }
            let full = |b: &str| format!("{}{b}", a.prefix);
            // Attribute bits that differ; skip the attribute if none do.
            let mine: Vec<BitPos> = settings[0].bits.iter().filter_map(|b| pos.get(full(b).as_str()).copied()).collect();
            if mine.len() != settings[0].bits.len() || mine.iter().all(|p| p.get(q) == p.get(&r)) {
                continue;
            }
            // The setting whose written bits match the image best, all cared-for bits equal.
            let best = settings
                .iter()
                .filter(|s| s.bits.iter().zip(&s.values).all(|(b, &(v, cares))| !cares || pos[full(b).as_str()].get(q) == v))
                .max_by_key(|s| s.bits.iter().zip(&s.values).filter(|(b, &(v, _))| pos[full(b).as_str()].get(q) == v).count());
            if let Some(s) = best {
                for (b, &(v, _)) in s.bits.iter().zip(&s.values) {
                    pos[full(b).as_str()].set(&mut r, v);
                }
                writeln!(text, "attr\t{u}\t{}\t{}", a.path, s.name)?;
                c[0] += 1;
            }
        }
        for d in direct_attrs(db, t) {
            let names = db.bcm.direct_bits(t, &d).unwrap();
            let ps: Option<Vec<BitPos>> = names.iter().map(|n| pos.get(n.as_str()).copied()).collect();
            let Some(ps) = ps else { continue };
            if ps.iter().all(|p| p.get(q) == p.get(&r)) {
                continue;
            }
            let v: String = ps.iter().map(|p| if p.get(q) { '1' } else { '0' }).collect();
            for p in &ps {
                p.set(&mut r, p.get(q));
            }
            writeln!(text, "direct\t{u}\t{d}\t{v}")?;
            c[1] += 1;
        }
        for (n, p) in bits {
            if p.get(q) != p.get(&r) {
                p.set(&mut r, p.get(q));
                writeln!(text, "bit\t{u}\t{n}\t{}", p.get(q) as u8)?;
                c[2] += 1;
            }
        }
    }
    let mut raws = 0;
    for (o, pt, start, n) in PARTS {
        for i in *start..start + n {
            if q[i] == r[i] {
                continue;
            }
            for bit in 0..8u8 {
                if (q[i] ^ r[i]) >> bit & 1 == 1 && !covered.contains(&(i, bit)) {
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
    let mut img = blank::image(db)?;
    let lines: Vec<Vec<&str>> = design.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).map(|l| l.split('\t').collect()).collect();
    let specs: Vec<String> = lines.iter().filter(|f| f[0] == "pin").map(|f| f[1].to_owned()).collect();
    let pins = pins::parse(db, &specs)?;
    let rows = pins::table(db, &pins)?;
    crate::write_rows(db, &mut img, &rows)?;
    let idx = graph.index();
    let mut edges = Vec::new();
    for f in lines.iter().filter(|f| f[0] == "edge") {
        let (s, d) = (idx.get(f[1]).with_context(|| format!("no node {}", f[1]))?, idx.get(f[2]).with_context(|| format!("no node {}", f[2]))?);
        edges.push(graph.edge(*s, *d).with_context(|| format!("no edge {} -> {}", f[1], f[2]))?);
    }
    let mut join = Join::new(db, graph);
    write_edges(&mut join, &edges, &mut img)?;
    drop(join);
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
