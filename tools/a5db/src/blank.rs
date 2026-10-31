//! The device's idle state: what every block instance holds when a design
//! doesn't use it, and the image built from that alone (README (n)).

use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::db::{Db, PARTS};

/// Named-bit values of every instance of a type, in `bit_names` order.
pub fn values(db: &Db, btype: &str, img: &[u8]) -> Vec<(String, Vec<bool>)> {
    let map = db.map(btype);
    map.uids()
        .into_iter()
        .map(|u| {
            let v = map.bit_names(u).iter().map(|b| map.pos(u, b).unwrap().get(img)).collect();
            (u.to_owned(), v)
        })
        .collect()
}

/// Per type: how many instances agree with the majority vector, and every
/// instance that doesn't, with its differing bits.
pub fn survey(db: &mut Db, imgs: &[(String, Vec<u8>)]) -> Result<()> {
    for t in db.types()? {
        db.load(&t)?;
        for (name, img) in imgs {
            let vals = values(db, &t, img);
            let mut count: HashMap<&[bool], usize> = HashMap::new();
            for (_, v) in &vals {
                *count.entry(v).or_default() += 1;
            }
            let (maj, n) = count.iter().max_by_key(|(_, n)| **n).map(|(v, n)| (v.to_vec(), *n)).unwrap();
            println!("{t}\t{name}\t{} instances, {} distinct, majority {n}, majority ones {}", vals.len(), count.len(), maj.iter().filter(|&&b| b).count());
            let map = db.map(&t);
            for (u, v) in &vals {
                if *v != maj {
                    let names = map.bit_names(u);
                    let d: Vec<&str> = names.iter().zip(v.iter().zip(&maj)).filter(|(_, (a, b))| a != b).map(|(n, _)| *n).collect();
                    let show: Vec<&str> = d.iter().take(6).copied().collect();
                    println!("  {u}\t{} differ\t{}", d.len(), show.join(" "));
                }
            }
        }
    }
    Ok(())
}

/// Set image bits per part that no named bit covers.
pub fn uncovered(db: &mut Db, img: &[u8]) -> Result<()> {
    let mut cov = vec![0u8; img.len()];
    for t in db.types()? {
        db.load(&t)?;
        let map = db.map(&t);
        for u in map.uids() {
            for b in map.bit_names(u) {
                let p = map.pos(u, b).unwrap();
                cov[p.byte] |= 1 << p.bit;
            }
        }
    }
    for (o, p, start, n) in PARTS {
        let (mut set, mut named, mut covered) = (0usize, 0usize, 0usize);
        let mut first = Vec::new();
        for i in *start..start + n {
            covered += cov[i].count_ones() as usize;
            set += img[i].count_ones() as usize;
            named += (img[i] & cov[i]).count_ones() as usize;
            if img[i] & !cov[i] != 0 && first.len() < 8 {
                first.push(format!("{:#x}:{:02x}", i - start, img[i] & !cov[i]));
            }
        }
        println!("{o}/{p}\tbits {}\tcovered {covered}\tset {set}\tset+named {named}\tset unnamed {}\t{}", n * 8, set - named, first.join(" "));
    }
    Ok(())
}

/// Covered (named) image bits, as a per-byte mask.
fn named_mask(db: &mut Db, len: usize) -> Result<Vec<u8>> {
    let mut cov = vec![0u8; len];
    for t in db.types()? {
        db.load(&t)?;
        let map = db.map(&t);
        for u in map.uids() {
            for b in map.bit_names(u) {
                let p = map.pos(u, b).unwrap();
                cov[p.byte] |= 1 << p.bit;
            }
        }
    }
    Ok(cov)
}

/// Derive the idle state from Quartus images of designs that use different
/// blocks: per type the majority vector; per instance the type's vector if
/// any image shows it, else the instance's own most common vector when at
/// least two images agree on it (a position-dependent idle value; the other
/// images' designs use the instance). Anything else is reported, not guessed.
/// Writes `idle.tsv` (type, bit: named bits idle at 1), `idle_inst.tsv`
/// (uid, bit, value: per-instance exceptions) and `idle_unnamed.tsv`
/// (object, part, bit offset: image bits no named bit covers, set in most
/// images).
pub fn derive(db: &mut Db, imgs: &[(String, Vec<u8>)], out: &std::path::Path) -> Result<()> {
    use std::fmt::Write;
    let (mut idle, mut inst) = (String::from("type\tbit\n"), String::from("uid\tbit\tvalue\n"));
    for t in db.types()? {
        db.load(&t)?;
        let per: Vec<Vec<(String, Vec<bool>)>> = imgs.iter().map(|(_, img)| values(db, &t, img)).collect();
        let mut count: HashMap<&[bool], usize> = HashMap::new();
        for vals in &per {
            for (_, v) in vals {
                *count.entry(v).or_default() += 1;
            }
        }
        let maj: Vec<bool> = count.iter().max_by_key(|(_, n)| **n).map(|(v, _)| v.to_vec()).unwrap();
        let map = db.map(&t);
        let first = map.uids()[0];
        for (b, _) in map.bit_names(first).iter().zip(&maj).filter(|(_, v)| **v) {
            writeln!(idle, "{t}\t{b}")?;
        }
        let (mut own, mut unsure) = (0, 0);
        for (k, (u, _)) in per[0].iter().enumerate() {
            let obs: Vec<&Vec<bool>> = per.iter().map(|vals| &vals[k].1).collect();
            if obs.iter().any(|v| **v == maj) {
                continue;
            }
            let names = map.bit_names(u);
            // The instance's own most common vector, if at least two images
            // agree on it and no other vector ties it: a design used it in
            // the others.
            let mut seen: Vec<(&Vec<bool>, usize)> = Vec::new();
            for v in &obs {
                match seen.iter_mut().find(|(w, _)| w == v) {
                    Some(e) => e.1 += 1,
                    None => seen.push((v, 1)),
                }
            }
            seen.sort_by_key(|e| std::cmp::Reverse(e.1));
            if seen[0].1 >= 2 && seen.get(1).is_none_or(|e| e.1 < seen[0].1) {
                own += 1;
                for ((b, v), m) in names.iter().zip(seen[0].0).zip(&maj) {
                    if v != m {
                        writeln!(inst, "{u}\t{b}\t{}", *v as u8)?;
                    }
                }
            } else {
                unsure += 1;
                let d: Vec<String> = obs
                    .iter()
                    .zip(imgs)
                    .map(|(v, (n, _))| format!("{n}: {} differ", v.iter().zip(&maj).filter(|(a, b)| a != b).count()))
                    .collect();
                eprintln!("  {t} {u}: never at the type's idle vector and not constant ({})", d.join(", "));
            }
        }
        eprintln!("{t}: {} instances, {} idle ones, {own} with their own idle vector, {unsure} undecided", per[0].len(), maj.iter().filter(|&&b| b).count());
    }
    let cov = named_mask(db, imgs[0].1.len())?;
    let mut unnamed = String::from("object\tpart\toffset\n");
    for (o, p, start, n) in PARTS {
        for i in *start..start + n {
            let sets: Vec<u8> = imgs.iter().map(|(_, img)| img[i] & !cov[i]).collect();
            for bit in 0..8 {
                let on = sets.iter().filter(|s| *s >> bit & 1 == 1).count();
                // Idle at 1 when most images agree; the others' designs clear it.
                if 2 * on > imgs.len() {
                    writeln!(unnamed, "{o}\t{p}\t{}", (i - start) * 8 + bit)?;
                }
                if on > 0 && on < imgs.len() {
                    eprintln!("  unnamed bit {o}/{p} {} set in {on} of {} images", (i - start) * 8 + bit, imgs.len());
                }
            }
        }
    }
    crate::db::write_table(&out.join("idle.tsv"), &idle)?;
    crate::db::write_table(&out.join("idle_inst.tsv"), &inst)?;
    crate::db::write_table(&out.join("idle_unnamed.tsv"), &unnamed)?;
    Ok(())
}

/// The idle image: every mapped bit of every instance at its idle value,
/// plus the unnamed constant bits. Parts no map covers stay zero.
pub fn image(db: &mut Db) -> Result<Vec<u8>> {
    let len = PARTS.iter().map(|(_, _, s, n)| s + n).max().unwrap();
    let mut img = vec![0u8; len];
    let mut ones: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for l in crate::db::read_table(&db.dir.join("idle.tsv"))?.lines().skip(1) {
        let (t, b) = l.split_once('\t').unwrap_or((l, ""));
        ones.entry(t.to_owned()).or_default().insert(b.to_owned());
    }
    for t in db.types()? {
        db.load(&t)?;
        let map = db.map(&t);
        let empty = Default::default();
        let on = ones.get(&t).unwrap_or(&empty);
        for u in map.uids() {
            for b in map.bit_names(u) {
                map.pos(u, b).unwrap().set(&mut img, on.contains(b));
            }
        }
    }
    for l in crate::db::read_table(&db.dir.join("idle_inst.tsv"))?.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        let t = db.type_of(f[0])?;
        let p = db.map(&t).pos(f[0], f[1]).with_context(|| format!("{} {} is not mapped", f[0], f[1]))?;
        p.set(&mut img, f[2] == "1");
    }
    for l in crate::db::read_table(&db.dir.join("idle_unnamed.tsv"))?.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        let (_, _, start, _) = PARTS.iter().find(|(o, p, ..)| *o == f[0] && *p == f[1]).context("unknown part")?;
        let off: usize = f[2].parse()?;
        img[start + off / 8] |= 1 << (off % 8);
    }
    Ok(img)
}

/// Bits that differ between two images, per part, with the instances that
/// own them (or "unnamed").
pub fn diff(db: &mut Db, a: &[u8], b: &[u8]) -> Result<()> {
    let mut owner: HashMap<(usize, u8), String> = HashMap::new();
    let changed: std::collections::HashSet<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
    for t in db.types()? {
        db.load(&t)?;
        let map = db.map(&t);
        for u in map.uids() {
            for n in map.bit_names(u) {
                let p = map.pos(u, n).unwrap();
                if changed.contains(&p.byte) && (a[p.byte] ^ b[p.byte]) >> p.bit & 1 == 1 {
                    owner.insert((p.byte, p.bit), format!("{u}\t{n}"));
                }
            }
        }
    }
    let mut by_inst: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for &i in &changed {
        for bit in 0..8u8 {
            if (a[i] ^ b[i]) >> bit & 1 == 1 {
                let (o, p, s, _) = PARTS.iter().find(|(_, _, s, n)| (*s..s + n).contains(&i)).unwrap();
                let who = owner.get(&(i, bit)).cloned().unwrap_or_else(|| format!("unnamed {o}/{p}\t{}", (i - s) * 8 + bit as usize));
                let (inst, name) = who.split_once('\t').unwrap();
                by_inst.entry(inst.to_owned()).or_default().push(format!("{name}={}", b[i] >> bit & 1));
            }
        }
    }
    for (inst, bits) in &by_inst {
        println!("{inst}\t{}\t{}", bits.len(), bits.join(" "));
    }
    println!("total\t{}", by_inst.values().map(Vec::len).sum::<usize>());
    Ok(())
}
