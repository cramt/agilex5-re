//! a5db: read and write named bits and attributes of an Agilex 5 (sm4revb)
//! `.sof` image through the database in db/sm4revb, without Quartus.
//!
//!   a5db attrs <db> <in.sof> <uid>                 decoded attributes
//!   a5db bits  <db> <in.sof> <uid>                 named bit values
//!   a5db set   <db> <in.sof> <out.sof> <hmac.key> <uid> <attr>=<value>...
//!   a5db rbf   <db> <in.sof> <out.rbf> <quartus root> <keyed-hash.key>
//!       the .rbf quartus_pfg would make (CORE all literal); the root is the
//!       install's version directory, the key `tools/bitstream.py key`'s
//!   a5db join  <db> <graph>                       how many graph edges resolve to bits
//!   a5db route <db> <graph> <in.sof> <out.sof> <hmac.key> <routes>
//!                                                  write the routes' mux bits
//!   a5db routecheck <db> <graph> <in.sof> <routes> [<out.sof> <hmac.key>]
//!       clear every routing bit of in.sof, set only the routes' bits, and
//!       compare with in.sof (its routes, e.g. from tools/rgdump/routes.tcl)
//!
//! <graph> is a tools/rgdump directory; <routes> is routes.tcl output or one
//! path of node names per line.
//!
//! CUSTOM values are setting names; DIRECT values are bit strings, MSB first
//! (asm2's `get_attribute` format). `hmac.key` is the `.sof` HMAC key that
//! `tools/sof.py key` derives from the local Quartus install.

mod db;
mod graph;
mod rbf;
mod route;
mod sof;

use anyhow::{bail, Context, Result};
use db::{Attr, Db};

/// The decoded value of one CUSTOM attribute: the settings whose cared-for
/// bit values all match, "?" if a bit is unmapped or nothing matches.
fn decode(db: &Db, btype: &str, img: &[u8], uid: &str, a: &Attr) -> Result<Option<String>> {
    let settings = db.bcm.settings(a);
    if settings.is_empty() {
        return Ok(None);
    }
    let map = db.map(btype);
    let mut found = Vec::new();
    'setting: for s in settings {
        for (b, &(v, cares)) in s.bits.iter().zip(&s.values) {
            if !cares {
                continue;
            }
            match map.pos(uid, &format!("{}{b}", a.prefix)) {
                Some(p) if p.get(img) == v => {}
                _ => continue 'setting,
            }
        }
        found.push(s.name.clone());
    }
    Ok(Some(if found.is_empty() { "?".into() } else { found.join("|") }))
}

fn attrs(dbdir: &str, sof_path: &str, uid: &str) -> Result<()> {
    let mut db = Db::open(dbdir)?;
    let sof = sof::Sof::read(sof_path)?;
    let btype = db.type_of(uid)?;
    for a in db.bcm.attributes(&btype) {
        if let Some(v) = decode(&db, &btype, sof.image(), uid, &a)? {
            println!("{}\t{v}", a.path);
        }
    }
    for d in direct_attrs(&db, &btype) {
        let bits = db.bcm.direct_bits(&btype, &d).unwrap().to_vec();
        let map = db.map(&btype);
        let s: String = bits
            .iter()
            .map(|b| map.pos(uid, b).map_or('?', |p| if p.get(sof.image()) { '1' } else { '0' }))
            .collect();
        println!("{d}\t{s}");
    }
    Ok(())
}

/// DIRECT attribute paths known for a block type.
fn direct_attrs(db: &Db, btype: &str) -> Vec<String> {
    let text = std::fs::read_to_string(db.dir.join("direct.tsv")).unwrap_or_default();
    text.lines().skip(1).filter_map(|l| {
        let mut f = l.split('\t');
        (f.next()? == btype).then(|| f.next().map(str::to_owned)).flatten()
    }).collect()
}

fn bits(dbdir: &str, sof_path: &str, uid: &str) -> Result<()> {
    let mut db = Db::open(dbdir)?;
    let sof = sof::Sof::read(sof_path)?;
    let btype = db.type_of(uid)?;
    let names: Vec<String> = db::read_table(&db.dir.join("bits").join(format!("{btype}.layout.tsv")))?
        .lines()
        .skip(1)
        .filter_map(|l| l.split('\t').nth(1).map(str::to_owned))
        .collect();
    let map = db.map(&btype);
    for n in names {
        if let Some(p) = map.pos(uid, &n) {
            println!("{n}\t{}", p.get(sof.image()) as u8);
        }
    }
    Ok(())
}

fn set(dbdir: &str, inp: &str, out: &str, key: &str, uid: &str, assigns: &[String]) -> Result<()> {
    let mut db = Db::open(dbdir)?;
    let mut sof = sof::Sof::read(inp)?;
    let btype = db.type_of(uid)?;
    let attrs = db.bcm.attributes(&btype);
    for asg in assigns {
        let (path, value) = asg.split_once('=').with_context(|| format!("{asg}: expected attr=value"))?;
        if let Some(bits) = db.bcm.direct_bits(&btype, path).map(<[String]>::to_vec) {
            if value.len() != bits.len() || !value.bytes().all(|c| c == b'0' || c == b'1') {
                bail!("{path} takes {} bits, MSB first", bits.len());
            }
            let map = db.map(&btype);
            for (b, c) in bits.iter().zip(value.bytes()) {
                map.pos(uid, b).with_context(|| format!("{b} is not mapped"))?.set(sof.image_mut(), c == b'1');
            }
            continue;
        }
        let a = attrs.iter().find(|a| a.path == path).with_context(|| format!("{btype} has no attribute {path}"))?;
        let s = db
            .bcm
            .settings(a)
            .iter()
            .find(|s| s.name == value)
            .with_context(|| format!("{path} has no setting {value}"))?;
        let writes: Vec<(String, bool)> = s.bits.iter().zip(&s.values).map(|(b, &(v, _))| (format!("{}{b}", a.prefix), v)).collect();
        let map = db.map(&btype);
        for (b, v) in writes {
            map.pos(uid, &b).with_context(|| format!("{b} is not mapped"))?.set(sof.image_mut(), v);
        }
    }
    let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
    sof.write(out, &key)
}

fn join(dbdir: &str, gdir: &str) -> Result<()> {
    let (mut db, g) = route::open(dbdir, gdir)?;
    let mut j = graph::Join::new(&mut db, &g);
    let u = route::universe(&mut j, &g)?;
    println!("edges with a mux setting: {}, as {} (block, setting) pairs", u.edges, u.pairs);
    println!("resolved to bits: {} edges; {} distinct routing bits", u.resolved, u.bits.len());
    for (m, n) in &u.misses {
        println!("  unresolved {n:9} {m:?}");
    }
    Ok(())
}

fn route_cmd(dbdir: &str, gdir: &str, inp: &str, routes: &str, out: Option<(&str, &str)>, check: bool) -> Result<()> {
    let (mut db, g) = route::open(dbdir, gdir)?;
    let mut sof = sof::Sof::read(inp)?;
    let paths = route::read_routes(routes)?;
    let idx = g.index();
    let mut j = graph::Join::new(&mut db, &g);
    let orig = sof.image().to_vec();
    let u = if check { Some(route::universe(&mut j, &g)?) } else { None };
    if let Some(u) = &u {
        route::clear(u, sof.image_mut());
    }
    let a = route::apply(&mut j, &g, &idx, &paths, sof.image_mut())?;
    println!("{} paths, {} hops; {} configured edges written, {} pass-through", paths.len(), a.hops, a.configured, a.pass_through);
    for n in &a.unknown_node {
        println!("  node not in the graph: {n}");
    }
    for (s, d) in &a.no_edge {
        println!("  no edge {s} -> {d}");
    }
    for (m, n) in &a.misses {
        println!("  unresolved {n} {m:?}");
    }
    let mut bad = !a.no_edge.is_empty() || !a.unknown_node.is_empty() || !a.misses.is_empty();
    if let Some(u) = &u {
        let img = sof.image();
        let (mut set_q, mut set_p, mut both) = (0, 0, 0);
        let mut diff = std::collections::HashSet::new();
        for (&(byte, bit), &equal) in &u.bits {
            let p = db::BitPos { byte, bit, equal };
            let (q, pr) = (p.get(&orig), p.get(img));
            set_q += q as usize;
            set_p += pr as usize;
            both += (q && pr) as usize;
            if q != pr {
                diff.insert((byte, bit));
            }
        }
        println!("routing bits: {} in the device; set by Quartus {set_q}, by the join {set_p}, both {both}", u.bits.len());
        let names = route::name_bits(&mut j, &g, &diff)?;
        let mut by: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        for &(byte, bit) in &diff {
            let equal = u.bits[&(byte, bit)];
            let q = db::BitPos { byte, bit, equal }.get(&orig);
            let e = by.entry(names.get(&(byte, bit)).cloned().unwrap_or_default()).or_default();
            if q { e.0 += 1 } else { e.1 += 1 }
        }
        for (k, (q, p)) in &by {
            println!("  differs: {k}: {q} set only by Quartus, {p} only by the join");
        }
        let other = orig.iter().zip(img).filter(|(a, b)| a != b).count();
        println!("image bytes differing from {inp}: {other}");
        bad |= !diff.is_empty();
    }
    if let Some((out, key)) = out {
        let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
        sof.write(out, &key)?;
    }
    if bad {
        std::process::exit(1);
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["attrs", dbdir, sof, uid] => attrs(dbdir, sof, uid),
        ["bits", dbdir, sof, uid] => bits(dbdir, sof, uid),
        ["set", dbdir, inp, out, key, uid, rest @ ..] if !rest.is_empty() => {
            set(dbdir, inp, out, key, uid, &args[6..])
        }
        ["join", dbdir, g] => join(dbdir, g),
        ["rbf", dbdir, inp, out, root, key] => {
            let mut db = Db::open(dbdir)?;
            let inst = rbf::install::Install::open(std::path::Path::new(root))?;
            let sof = sof::Sof::read(inp)?;
            let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
            std::fs::write(out, rbf::write(&mut db, &inst, &sof, &key)?).with_context(|| format!("writing {out}"))
        }
        ["route", dbdir, g, inp, out, key, routes] => route_cmd(dbdir, g, inp, routes, Some((out, key)), false),
        ["routecheck", dbdir, g, inp, routes] => route_cmd(dbdir, g, inp, routes, None, true),
        ["routecheck", dbdir, g, inp, routes, out, key] => route_cmd(dbdir, g, inp, routes, Some((out, key)), true),
        _ => {
            eprintln!("{}", include_str!("main.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//! ").trim_start_matches("//!")).collect::<Vec<_>>().join("\n"));
            std::process::exit(2)
        }
    }
}
