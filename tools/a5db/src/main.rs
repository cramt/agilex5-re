//! a5db: read and write named bits and attributes of an Agilex 5 (sm4revb)
//! `.sof` image through the database in db/sm4revb, without Quartus.
//!
//!   a5db attrs <db> <in.sof> <uid>                 decoded attributes
//!   a5db bits  <db> <in.sof> <uid>                 named bit values
//!   a5db set   <db> <in.sof> <out.sof> <hmac.key> <uid> <attr>=<value>...
//!
//! CUSTOM values are setting names; DIRECT values are bit strings, MSB first
//! (asm2's `get_attribute` format). `hmac.key` is the `.sof` HMAC key that
//! `tools/sof.py key` derives from the local Quartus install.

mod db;
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
    let names: Vec<String> = std::fs::read_to_string(db.dir.join("bits").join(format!("{btype}.layout.tsv")))?
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

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["attrs", dbdir, sof, uid] => attrs(dbdir, sof, uid),
        ["bits", dbdir, sof, uid] => bits(dbdir, sof, uid),
        ["set", dbdir, inp, out, key, uid, rest @ ..] if !rest.is_empty() => {
            set(dbdir, inp, out, key, uid, &args[6..])
        }
        _ => {
            eprintln!("{}", include_str!("main.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//! ").trim_start_matches("//!")).collect::<Vec<_>>().join("\n"));
            std::process::exit(2)
        }
    }
}
