//! a5db: read and write named bits and attributes of an Agilex 5 (sm4revb)
//! `.sof` image through the database in db/sm4revb, without Quartus.
//!
//!   a5db attrs <db> <in.sof> <uid>                 decoded attributes
//!   a5db bits  <db> <in.sof> <uid>                 named bit values
//!   a5db set   <db> <in.sof> <out.sof> <hmac.key> <uid> <attr>=<value>...
//!   a5db rbf   <db> <in.sof> <out.rbf> <quartus root> <keyed-hash.key>
//!       the .rbf quartus_pfg would make, byte for byte; the root is the
//!       install's version directory, the key `tools/bitstream.py key`'s
//!   a5db join  <db> <graph>                       how many graph edges resolve to bits
//!   a5db route <db> <graph> <in.sof> <out.sof> <hmac.key> <routes>
//!                                                  write the routes' mux bits
//!   a5db fanout <graph> <name prefix>...          edges out of matching nodes
//!   a5db fanin <graph> <name prefix>...           edges into matching nodes
//!   a5db routed <db> <graph> <in.sof>             edges the image's mux settings select
//!   a5db net   <db> <graph> <in.sof> <out.sof> <hmac.key> '[clock:]from,from...>to;to...'...
//!       route nets through free nodes (sources may be busy: they are the
//!       net's own), one tree per net, and write their bits; a clock: net
//!       avoids H/V/LL/LEIM
//!   a5db routecheck <db> <graph> <in.sof> <routes> [<out.sof> <hmac.key>]
//!       clear every routing bit of in.sof, set only the routes' bits, and
//!       compare with in.sof (its routes, e.g. from tools/rgdump/routes.tcl)
//!
//!   a5db blank <db> <quartus root> <hmac.key> <out.sof>
//!       the idle image (README (n)) in a .sof that a5db builds itself
//!   a5db wrap <quartus root> <hmac.key> <in.sof> <out.sof>
//!       in.sof's image in a5db's own .sof container
//!   a5db apply <db> <in.sof> <out.sof> <hmac.key> <table.tsv>...
//!       write named bits from (uid, bit, value) tables (db/sm4revb/io/*)
//!   a5db pintable <db> <ball>=<in|in0|in1|out|clock_in|in_reg|out_reg>[:<3.3|1.8|1.1>]...
//!       the I/O bits of a set of used pins, from db/sm4revb/io/template.tsv
//!   a5db pins <db> <in.sof> <out.sof> <hmac.key> <ball>=<use>[:<std>]...
//!       write them
//!   a5db extract <db> <graph> <in.sof> [<ball>=<use>[:<std>]...]
//!       the image as a design file: pins, routing edges, attributes
//!       (src/rebuild.rs)
//!   a5db build <db> <graph> <quartus root> <hmac.key> <design> <out.sof>
//!   a5db rebuild <db> <graph> <quartus root> <hmac.key> <in.sof> <out dir> [pins...]
//!       extract, build from the written file, and print the bits that differ
//!   a5db pinmap <db> <graph>                    every ball's boundary nodes (db/sm4revb/io/pin_nodes.tsv)
//!   a5db pincheck <db> <graph> <design>...      do a design file's pins start/end at the predicted nodes?
//!   a5db trace <graph> <node> <depth>              downstream edges, breadth first
//!   a5db traceback <graph> <node> <depth>          upstream edges, breadth first
//!   a5db idle-derive <db> <sof>...                 derive db/sm4revb/idle*.tsv
//!   a5db idle-diff <db> <in.sof>                   bits of in.sof that differ from idle
//!   a5db imgdiff <db> <a.sof> <b.sof>              bits that differ, by instance (b's values)
//!
//! <graph> is a tools/rgdump directory; <routes> is routes.tcl output or one
//! path of node names per line.
//!
//! CUSTOM values are setting names; DIRECT values are bit strings, MSB first
//! (asm2's `get_attribute` format). `hmac.key` is the `.sof` HMAC key that
//! `tools/sof.py key` derives from the local Quartus install.

mod blank;
mod db;
mod graph;
mod pinmap;
mod pins;
mod rebuild;
mod rbf;
mod route;
mod sof;
mod sofgen;

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
    let paths = route::read_routes(routes, &route::type_aliases(&g))?;
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
        for (&(byte, bit), &(equal, ..)) in &u.bits {
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
        let mut by: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        for &(byte, bit) in &diff {
            let equal = u.bits[&(byte, bit)].0;
            let q = db::BitPos { byte, bit, equal }.get(&orig);
            let e = by.entry(route::bit_name(u, &g, (byte, bit))).or_default();
            if q { e.0 += 1 } else { e.1 += 1 }
        }
        for (k, (q, p)) in &by {
            println!("  differs: {k}: {q} set only by Quartus, {p} only by the join");
        }
        if !diff.is_empty() {
            // Which decoded edges of the Quartus image account for the rest.
            let edges = route::decode(&mut j, &g, &orig)?;
            let mut left = diff.clone();
            let mut used = Vec::new();
            for &e in &edges {
                if let Some(Ok(w)) = j.edge(e)? {
                    let hit: Vec<_> = w.iter().map(|(p, _)| (p.byte, p.bit)).filter(|b| left.contains(b)).collect();
                    if !hit.is_empty() {
                        hit.iter().for_each(|b| {
                            left.remove(b);
                        });
                        used.push(e);
                    }
                }
            }
            println!("decoded from the Quartus image: {} active edges; {} of the {} differing bits are theirs:", edges.len(), diff.len() - left.len(), diff.len());
            for e in used {
                let src = (0..g.names.len()).find(|&s| g.fanout(s as u32).contains(&e)).unwrap_or(0);
                println!("  edge {} -> {} ({} {})", g.names[src], g.names[g.dst[e] as usize], g.uids[g.uid[e] as usize], g.cfgs[g.cfg[e] as usize]);
            }
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

/// Write `uid, bit, value` rows (db/sm4revb/io tables, `pintable` output).
/// A uid `@object/part` sets an image bit no named bit covers, at that bit
/// offset of the part.
pub fn write_rows(db: &mut Db, img: &mut [u8], rows: &[(String, String, String)]) -> Result<()> {
    for (uid, bit, v) in rows {
        if let Some(op) = uid.strip_prefix('@') {
            let (o, pt) = op.split_once('/').with_context(|| format!("{op}: expected object/part"))?;
            let (_, _, start, _) = db::PARTS.iter().find(|(a, b, ..)| *a == o && *b == pt).with_context(|| format!("no part {op}"))?;
            let off: usize = bit.parse()?;
            let (byte, m) = (start + off / 8, 1u8 << (off % 8));
            img[byte] = if v == "1" { img[byte] | m } else { img[byte] & !m };
            continue;
        }
        let ty = db.type_of(uid)?;
        let p = db.map(&ty).pos(uid, bit).with_context(|| format!("{uid} {bit} is not mapped"))?;
        p.set(img, v == "1");
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
        ["survey", dbdir, sofs @ ..] if !sofs.is_empty() => {
            let mut db = Db::open(dbdir)?;
            let imgs = sofs.iter().map(|s| Ok((s.to_string(), sof::Sof::read(s)?.image().to_vec()))).collect::<Result<Vec<_>>>()?;
            blank::survey(&mut db, &imgs)
        }
        ["idle-derive", dbdir, sofs @ ..] if !sofs.is_empty() => {
            let mut db = Db::open(dbdir)?;
            let imgs = sofs.iter().map(|s| Ok((s.to_string(), sof::Sof::read(s)?.image().to_vec()))).collect::<Result<Vec<_>>>()?;
            let dir = db.dir.clone();
            blank::derive(&mut db, &imgs, &dir)
        }
        ["idle-diff", dbdir, s] => {
            let mut db = Db::open(dbdir)?;
            let img = blank::image(&mut db)?;
            blank::diff(&mut db, &img, sof::Sof::read(s)?.image())
        }
        ["blank", dbdir, root, key, out] => {
            let mut db = Db::open(dbdir)?;
            let inst = rbf::install::Install::open(std::path::Path::new(root))?;
            let img = blank::image(&mut db)?;
            let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
            sof::Sof::from_raw(sofgen::build(&inst, &img)?)?.write(out, &key)
        }
        ["wrap", root, key, inp, out] => {
            let inst = rbf::install::Install::open(std::path::Path::new(root))?;
            let img = sof::Sof::read(inp)?.image().to_vec();
            let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
            sof::Sof::from_raw(sofgen::build(&inst, &img)?)?.write(out, &key)
        }
        ["apply", dbdir, inp, out, key, tables @ ..] if !tables.is_empty() => {
            let mut db = Db::open(dbdir)?;
            let mut sof = sof::Sof::read(inp)?;
            for t in tables {
                let text = std::fs::read_to_string(t).with_context(|| format!("reading {t}"))?;
                let rows = text
                    .lines()
                    .map(|l| l.split('\t').collect::<Vec<_>>())
                    .filter(|f| f.len() >= 3 && !f[0].starts_with('#') && f[2] != "value")
                    .map(|f| (f[0].to_owned(), f[1].to_owned(), f[2].to_owned()))
                    .collect::<Vec<_>>();
                write_rows(&mut db, sof.image_mut(), &rows)?;
            }
            let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
            sof.write(out, &key)
        }
        ["extract", dbdir, g, inp, ..] => {
            let (mut db, g) = route::open(dbdir, g)?;
            let q = sof::Sof::read(inp)?;
            print!("{}", rebuild::extract(&mut db, &g, q.image(), &args[4..])?);
            Ok(())
        }
        ["build", dbdir, g, root, key, design, out] => {
            let (mut db, g) = route::open(dbdir, g)?;
            let inst = rbf::install::Install::open(std::path::Path::new(root))?;
            let text = std::fs::read_to_string(design).with_context(|| format!("reading {design}"))?;
            let img = rebuild::build(&mut db, &g, &text)?;
            let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
            sof::Sof::from_raw(sofgen::build(&inst, &img)?)?.write(out, &key)
        }
        ["rebuild", dbdir, g, root, key, inp, outdir, ..] => {
            let (mut db, g) = route::open(dbdir, g)?;
            let inst = rbf::install::Install::open(std::path::Path::new(root))?;
            let q = sof::Sof::read(inp)?;
            std::fs::create_dir_all(outdir)?;
            let text = rebuild::extract(&mut db, &g, q.image(), &args[7..])?;
            let dpath = std::path::Path::new(outdir).join("design.txt");
            std::fs::write(&dpath, &text)?;
            let img = rebuild::build(&mut db, &g, &std::fs::read_to_string(&dpath)?)?;
            let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
            sof::Sof::from_raw(sofgen::build(&inst, &img)?)?.write(&std::path::Path::new(outdir).join("rebuilt.sof").to_string_lossy(), &key)?;
            blank::diff(&mut db, &img, q.image())
        }
        ["pintable", dbdir, specs @ ..] if !specs.is_empty() => {
            let mut db = Db::open(dbdir)?;
            let pins = pins::parse(&db, &args[2..])?;
            println!("uid\tbit\tvalue");
            for (u, b, v) in pins::table(&mut db, &pins)? {
                println!("{u}\t{b}\t{v}");
            }
            Ok(())
        }
        ["pins", dbdir, inp, out, key, specs @ ..] if !specs.is_empty() => {
            let mut db = Db::open(dbdir)?;
            let mut sof = sof::Sof::read(inp)?;
            let pins = pins::parse(&db, &args[5..])?;
            let rows = pins::table(&mut db, &pins)?;
            write_rows(&mut db, sof.image_mut(), &rows)?;
            let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
            sof.write(out, &key)
        }
        ["pinmap", dbdir, g] => {
            let db = Db::open(dbdir)?;
            let g = graph::Graph::load(std::path::Path::new(g))?;
            let rev = pinmap::Rev::new(&g);
            let all = pinmap::balls(&db)?;
            let at = pinmap::nodes_at(&g, &all.iter().map(|b| (b.x, b.y, b.z)).collect::<Vec<_>>());
            println!("# Pin ↔ routing-graph node map (README (t)), written by `a5db pinmap`: for each");
            println!("# ball, the node where fabric routing starts (in0, in1, clock, direct) or ends");
            println!("# (out, oe, out2), and the unconfigured hops between it and the pad's buffer.");
            println!("ball\tuid\trole\tnode\thops");
            for b in &all {
                let pad = at.get(&(b.x, b.y, b.z)).cloned().unwrap_or_default();
                for (role, n, d) in pinmap::roles(&g, &rev, &pad) {
                    println!("{}\t{}\t{role}\t{}\t{d}", b.ball, b.uid, g.names[n as usize]);
                }
            }
            Ok(())
        }
        ["pincheck", dbdir, g, designs @ ..] if !designs.is_empty() => {
            let db = Db::open(dbdir)?;
            let g = graph::Graph::load(std::path::Path::new(g))?;
            let rev = pinmap::Rev::new(&g);
            let idx = g.index();
            let all = pinmap::balls(&db)?;
            let at = pinmap::nodes_at(&g, &all.iter().map(|b| (b.x, b.y, b.z)).collect::<Vec<_>>());
            for d in designs {
                let (mut outs, mut ins): (std::collections::HashSet<u32>, std::collections::HashSet<u32>) = Default::default();
                let mut specs = Vec::new();
                for l in std::fs::read_to_string(d)?.lines() {
                    let f: Vec<&str> = l.split('\t').collect();
                    match f[0] {
                        "pin" => specs.push(f[1].to_owned()),
                        "edge" => {
                            outs.insert(idx[f[1]]);
                            ins.insert(idx[f[2]]);
                        }
                        _ => {}
                    }
                }
                let (mut ok, mut n) = (0, 0);
                for s in &specs {
                    let (ball, rest) = s.split_once('=').unwrap();
                    let u = rest.split(':').next().unwrap();
                    let b = all.iter().find(|b| b.ball == ball).with_context(|| format!("no ball {ball}"))?;
                    let pad = at.get(&(b.x, b.y, b.z)).cloned().unwrap_or_default();
                    let roles = pinmap::roles(&g, &rev, &pad);
                    let io96 = b.uid.starts_with("pio");
                    let want = match u {
                        "in" | "in_reg" => if io96 { "in0" } else { "in1" },
                        "in0" | "in1" => u,
                        "clock_in" => "clock",
                        _ => "out",
                    };
                    let fwd = want != "out";
                    // Edges whose setting is idle or a pass-through (an RPI's input, the
                    // HIOL 4:1 data mux) are not in a design file: look up to 3 hops
                    // past the boundary, through non-wire nodes, for a listed edge.
                    let wire = |n: u32| { let t = g.names[n as usize].split(':').next().unwrap(); t == "LL" || t == "LAB_RE" || ((t.starts_with('H') || t.starts_with('V')) && t[1..].parse::<u32>().is_ok()) };
                    let used = |node: u32| {
                        let mut front = vec![node];
                        for _ in 0..4 {
                            let mut next = Vec::new();
                            for &m in &front {
                                if (fwd && outs.contains(&m)) || (!fwd && ins.contains(&m)) { return true; }
                                if fwd { for e in g.fanout(m) { if !wire(g.dst[e]) { next.push(g.dst[e]); } } }
                                else { for f in rev.fanin(m) { if !wire(rev.src[f]) { next.push(rev.src[f]); } } }
                            }
                            front = next;
                        }
                        false
                    };
                    let hits: Vec<&str> = roles.iter().filter(|r| (r.0 == "out") != fwd && r.0 != "oe" && r.0 != "out2" && used(r.1)).map(|r| r.0).collect();
                    let good = hits.contains(&want);
                    n += 1;
                    ok += good as usize;
                    println!("{d}\t{s}\t{want}\t{}\t{}", if good { "ok" } else { "MISS" }, hits.join(","));
                }
                eprintln!("{d}: {ok} of {n} pins use the predicted node");
            }
            Ok(())
        }
        ["trace", g, node, depth] => {
            let g = graph::Graph::load(std::path::Path::new(g))?;
            let idx = g.index();
            let mut frontier = vec![*idx.get(node).with_context(|| format!("no node {node}"))?];
            let mut seen: std::collections::HashSet<u32> = frontier.iter().copied().collect();
            for d in 0..depth.parse::<usize>()? {
                let mut next = Vec::new();
                for &n in &frontier {
                    for e in g.fanout(n) {
                        let t = g.dst[e];
                        println!("{d}\t{}\t{}\t{}\t{}", g.names[n as usize], g.names[t as usize], g.uids[g.uid[e] as usize], g.cfgs[g.cfg[e] as usize]);
                        if seen.insert(t) {
                            next.push(t);
                        }
                    }
                }
                frontier = next;
            }
            Ok(())
        }
        ["traceback", g, node, depth] => {
            let g = graph::Graph::load(std::path::Path::new(g))?;
            let idx = g.index();
            let mut rev: std::collections::HashMap<u32, Vec<(u32, usize)>> = Default::default();
            for s in 0..g.names.len() as u32 {
                for e in g.fanout(s) {
                    rev.entry(g.dst[e]).or_default().push((s, e));
                }
            }
            let mut frontier = vec![*idx.get(node).with_context(|| format!("no node {node}"))?];
            let mut seen: std::collections::HashSet<u32> = frontier.iter().copied().collect();
            for d in 0..depth.parse::<usize>()? {
                let mut next = Vec::new();
                for &n in &frontier {
                    for &(s, e) in rev.get(&n).map(Vec::as_slice).unwrap_or(&[]) {
                        println!("{d}\t{}\t{}\t{}\t{}", g.names[s as usize], g.names[n as usize], g.uids[g.uid[e] as usize], g.cfgs[g.cfg[e] as usize]);
                        if seen.insert(s) {
                            next.push(s);
                        }
                    }
                }
                frontier = next;
            }
            Ok(())
        }
        ["imgdiff", dbdir, a, b] => {
            let mut db = Db::open(dbdir)?;
            blank::diff(&mut db, sof::Sof::read(a)?.image(), sof::Sof::read(b)?.image())
        }
        ["uncovered", dbdir, s] => blank::uncovered(&mut Db::open(dbdir)?, sof::Sof::read(s)?.image()),
        ["fanout", g, prefixes @ ..] if !prefixes.is_empty() => {
            let g = graph::Graph::load(std::path::Path::new(g))?;
            for (n, name) in g.names.iter().enumerate() {
                if prefixes.iter().any(|p| name.starts_with(*p)) {
                    for e in g.fanout(n as u32) {
                        println!("{name}\t{}\t{}\t{}", g.names[g.dst[e] as usize], g.uids[g.uid[e] as usize], g.cfgs[g.cfg[e] as usize]);
                    }
                }
            }
            Ok(())
        }
        ["fanin", g, prefixes @ ..] if !prefixes.is_empty() => {
            let g = graph::Graph::load(std::path::Path::new(g))?;
            for s in 0..g.names.len() as u32 {
                for e in g.fanout(s) {
                    let name = &g.names[g.dst[e] as usize];
                    if prefixes.iter().any(|p| name.starts_with(*p)) {
                        println!("{name}\t{}\t{}\t{}", g.names[s as usize], g.uids[g.uid[e] as usize], g.cfgs[g.cfg[e] as usize]);
                    }
                }
            }
            Ok(())
        }
        ["net", dbdir, g, inp, out, key, nets @ ..] if !nets.is_empty() => {
            let (mut db, g) = route::open(dbdir, g)?;
            let mut sof = sof::Sof::read(inp)?;
            let idx = g.index();
            let mut j = graph::Join::new(&mut db, &g);
            let mut busy: std::collections::HashSet<u32> = route::decode(&mut j, &g, sof.image())?.into_iter().map(|e| g.dst[e]).collect();
            eprintln!("{} nodes driven in {inp}", busy.len());
            for spec in nets {
                let (clock, spec) = match spec.strip_prefix("clock:") {
                    Some(s) => (true, s),
                    None => (false, &spec[..]),
                };
                let (from, to) = spec.split_once('>').with_context(|| format!("{spec}: expected from,...>to;to..."))?;
                let node = |n: &str| idx.get(n).copied().with_context(|| format!("no node {n}"));
                // One tree per net: each sink's branch may start anywhere on
                // the tree so far (a clock's trunk is shared, not duplicated).
                let mut tree: Vec<u32> = from.split(',').map(node).collect::<Result<_>>()?;
                for to in to.split(';') {
                    let to = node(to)?;
                    let path = route::find_path(&mut j, &g, &tree, to, &busy, clock)?.with_context(|| format!("{spec}: no free path to {}", g.names[to as usize]))?;
                    println!("net {} > {}: {} hops", from, g.names[to as usize], path.len());
                    for &e in &path {
                        let s = route::edge_src(&g, e);
                        let w = j.edge(e)?;
                        let what = match &w {
                            Some(Ok(_)) => format!("{} {}", g.uids[g.uid[e] as usize], g.cfgs[g.cfg[e] as usize]),
                            Some(Err(m)) => format!("({m:?})"),
                            None => "(hardwired)".into(),
                        };
                        println!("  {} -> {}  {what}", g.names[s as usize], g.names[g.dst[e] as usize]);
                        if let Some(Ok(w)) = w {
                            for (p, v) in w {
                                p.set(sof.image_mut(), v);
                            }
                        }
                        busy.insert(g.dst[e]);
                        tree.push(g.dst[e]);
                    }
                }
            }
            let key = std::fs::read(key).with_context(|| format!("reading {key}"))?;
            sof.write(out, &key)
        }
        ["routed", dbdir, g, inp] => {
            let (mut db, g) = route::open(dbdir, g)?;
            let sof = sof::Sof::read(inp)?;
            let mut j = graph::Join::new(&mut db, &g);
            let edges = route::decode(&mut j, &g, sof.image())?;
            let mut src = vec![0u32; g.dst.len()];
            for s in 0..g.names.len() as u32 {
                for e in g.fanout(s) {
                    src[e] = s;
                }
            }
            for e in edges {
                println!("{}\t{}\t{}\t{}", g.names[src[e] as usize], g.names[g.dst[e] as usize], g.uids[g.uid[e] as usize], g.cfgs[g.cfg[e] as usize]);
            }
            Ok(())
        }
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
