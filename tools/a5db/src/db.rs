//! The sm4revb database under db/sm4revb: image layout, per-block-type bit
//! maps (one relative layout plus an anchor per instance), the BCM type tree
//! and CUSTOM settings, and DIRECT attributes' bit lists.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

pub const FRAME_BYTES: usize = 540;

/// (object, part, image offset, size) from sof_meta.pb (README (g)).
pub const PARTS: &[(&str, &str, usize, usize)] = &[
    ("perictrl0", "hdr", 0, 902),
    ("perictrl0", "data", 902, 4194304),
    ("io96_0", "hdr", 4195206, 5855),
    ("io96_0", "t40", 4201061, 16384),
    ("io96_0", "t41", 4217445, 529408),
    ("perictrl2", "hdr", 4746853, 902),
    ("perictrl2", "data", 4747755, 4194304),
    ("io96_1", "hdr", 8942059, 5855),
    ("io96_1", "t40", 8947914, 16384),
    ("io96_1", "t41", 8964298, 529408),
    ("perictrl1", "hdr", 9493706, 2658),
    ("perictrl1", "data", 9496364, 4194304),
    ("sector0", "hdr", 13690668, 412),
    ("sector0", "cram", 13691080, 2540160),
    ("sector0", "ram", 16231240, 491520),
    ("sector1", "hdr", 16722760, 284),
    ("sector1", "cram", 16723044, 2540160),
    ("sector1", "ram", 19263204, 491520),
];

/// Where one named bit sits in the image. `equal` is false when the image
/// holds the bit's inverse.
#[derive(Clone, Copy, Debug)]
pub struct BitPos {
    pub byte: usize,
    pub bit: u8,
    pub equal: bool,
}

impl BitPos {
    pub fn get(&self, img: &[u8]) -> bool {
        (img[self.byte] >> self.bit & 1 == 1) == self.equal
    }
    pub fn set(&self, img: &mut [u8], v: bool) {
        let m = 1u8 << self.bit;
        if v == self.equal {
            img[self.byte] |= m;
        } else {
            img[self.byte] &= !m;
        }
    }
}

/// One image part of an instance: CRAM is addressed by (frame, bdata),
/// anything else by a bit offset in `frame` (README, docs/db-format.md).
struct Anchor {
    start: usize,
    cram: bool,
    frame: i64,
    bdata: i64,
}

struct Placed {
    layout: usize,
    parts: HashMap<String, Anchor>,
}

pub struct BlockMap {
    layouts: Vec<HashMap<String, (String, i64, i64, bool)>>,
    anchors: HashMap<String, Placed>,
}

/// A database table: `path` itself, or `path.xz` when the table is stored
/// compressed (the big io96 and HSSI layouts).
pub fn read_table(path: &Path) -> Result<String> {
    if path.exists() {
        return std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()));
    }
    let mut xz = path.as_os_str().to_owned();
    xz.push(".xz");
    let file = std::fs::File::open(&xz).with_context(|| format!("reading {} (or .xz)", path.display()))?;
    let mut text = String::new();
    lzma_rust2::XzReader::new(std::io::BufReader::new(file), false)
        .read_to_string(&mut text)
        .with_context(|| format!("decompressing {}.xz", path.display()))?;
    Ok(text)
}

/// Write a database table, as `.tsv.xz` (`xz -9` equivalent) when it is
/// bigger than 4 MiB (docs/db-format.md).
pub fn write_table(path: &Path, text: &str) -> Result<()> {
    if text.len() <= 4 << 20 {
        let _ = std::fs::remove_file({ let mut x = path.as_os_str().to_owned(); x.push(".xz"); x });
        return std::fs::write(path, text).with_context(|| format!("writing {}", path.display()));
    }
    let mut xz = path.as_os_str().to_owned();
    xz.push(".xz");
    let _ = std::fs::remove_file(path);
    let status = std::process::Command::new("xz")
        .args(["-9", "-c"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::fs::File::create(&xz)?)
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(text.as_bytes())?;
            c.wait()
        })
        .with_context(|| format!("running xz for {}", path.display()))?;
    anyhow::ensure!(status.success(), "xz failed for {}", path.display());
    Ok(())
}

fn tsv(path: &Path) -> Result<Vec<Vec<String>>> {
    let text = read_table(path)?;
    Ok(text.lines().skip(1).map(|l| l.split('\t').map(str::to_owned).collect()).collect())
}

impl BlockMap {
    pub fn load(prefix: &Path) -> Result<BlockMap> {
        let mut layouts: Vec<HashMap<String, (String, i64, i64, bool)>> = Vec::new();
        for r in tsv(&prefix.with_extension("layout.tsv"))? {
            let id: usize = r[0].parse()?;
            if layouts.len() <= id {
                layouts.resize_with(id + 1, HashMap::new);
            }
            layouts[id].insert(r[1].clone(), (r[2].clone(), r[3].parse()?, r[4].parse()?, r[5] == "1"));
        }
        let mut anchors: HashMap<String, Placed> = HashMap::new();
        for r in tsv(&prefix.with_extension("anchors.tsv"))? {
            let (_, _, start, _) = PARTS
                .iter()
                .find(|(o, p, _, _)| *o == r[2] && *p == r[3])
                .with_context(|| format!("unknown image part {}/{}", r[2], r[3]))?;
            let placed = anchors
                .entry(r[0].clone())
                .or_insert_with(|| Placed { layout: r[1].parse().unwrap_or(0), parts: HashMap::new() });
            placed.parts.insert(
                r[3].clone(),
                Anchor { start: *start, cram: r[3] == "cram", frame: r[4].parse()?, bdata: r[5].parse()? },
            );
        }
        Ok(BlockMap { layouts, anchors })
    }

    pub fn has(&self, uid: &str) -> bool {
        self.anchors.contains_key(uid)
    }

    /// Every instance, sorted.
    pub fn uids(&self) -> Vec<&str> {
        let mut u: Vec<&str> = self.anchors.keys().map(String::as_str).collect();
        u.sort();
        u
    }

    /// The named bits of an instance's layout, sorted.
    pub fn bit_names(&self, uid: &str) -> Vec<&str> {
        let Some(placed) = self.anchors.get(uid) else { return Vec::new() };
        let mut b: Vec<&str> = self.layouts[placed.layout].keys().map(String::as_str).collect();
        b.sort();
        b
    }

    /// Image byte offsets of every mapped bit of an instance in one part.
    pub fn byte_offsets(&self, uid: &str, part: &str) -> Vec<usize> {
        let Some(placed) = self.anchors.get(uid) else { return Vec::new() };
        let Some(a) = placed.parts.get(part) else { return Vec::new() };
        self.layouts[placed.layout]
            .values()
            .filter(|(p, ..)| p == part)
            .map(|(_, df, dd, _)| {
                let (f, d) = ((a.frame + df) as usize, (a.bdata + dd) as usize);
                if a.cram { a.start + f * FRAME_BYTES + d / 8 } else { a.start + f / 8 }
            })
            .collect()
    }

    pub fn pos(&self, uid: &str, bit: &str) -> Option<BitPos> {
        let placed = self.anchors.get(uid)?;
        let (part, df, dd, equal) = self.layouts[placed.layout].get(bit)?;
        let a = placed.parts.get(part)?;
        let (f, d) = ((a.frame + df) as usize, (a.bdata + dd) as usize);
        Some(if a.cram {
            BitPos { byte: a.start + f * FRAME_BYTES + d / 8, bit: (d % 8) as u8, equal: *equal }
        } else {
            BitPos { byte: a.start + f / 8, bit: (f % 8) as u8, equal: *equal }
        })
    }
}

/// One CUSTOM setting: the leaf intent's bits and their values; `None` is a
/// value the API prints as `<v>` (kept as don't-care when decoding, written
/// as `v`).
pub struct Setting {
    pub name: String,
    pub bits: Vec<String>,
    pub values: Vec<(bool, bool)>, // (value, cares)
}

#[derive(Clone)]
pub struct Attr {
    pub path: String,
    pub prefix: String,
    pub leaf: String,
    pub name: String,
}

pub struct Bcm {
    types: serde_json::Value,
    settings: HashMap<(String, String), Vec<Setting>>,
    direct: HashMap<(String, String), Vec<String>>,
}

/// Split a Tcl list of words, where `{…}` groups one word.
fn tcl_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ' ' if depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

impl Bcm {
    pub fn load(dir: &Path) -> Result<Bcm> {
        let types: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("bcm_types.json"))?)?;
        let mut settings: HashMap<(String, String), Vec<Setting>> = HashMap::new();
        for r in tsv(&dir.join("bcm_settings.tsv"))? {
            let values = r[4]
                .split_whitespace()
                .map(|v| {
                    let cares = !v.starts_with('<');
                    let v = v.trim_matches(|c| c == '<' || c == '>');
                    (v == "1", cares)
                })
                .collect();
            settings
                .entry((r[0].clone(), r[1].clone()))
                .or_default()
                .push(Setting { name: r[2].clone(), bits: tcl_words(&r[3]), values });
        }
        let mut direct = HashMap::new();
        for r in tsv(&dir.join("direct.tsv"))? {
            direct.insert((r[0].clone(), r[1].clone()), r[2].split(' ').map(str::to_owned).collect());
        }
        Ok(Bcm { types: types["types"].clone(), settings, direct })
    }

    /// Every CUSTOM attribute under a block type, depth first.
    pub fn attributes(&self, btype: &str) -> Vec<Attr> {
        let mut out = Vec::new();
        self.walk(btype, "", &mut out);
        out
    }

    fn walk(&self, t: &str, prefix: &str, out: &mut Vec<Attr>) {
        let node = &self.types[t];
        if let Some(cs) = node["custom"].as_array() {
            for a in cs.iter().filter_map(|a| a.as_str()) {
                out.push(Attr { path: format!("{prefix}{a}"), prefix: prefix.to_owned(), leaf: t.to_owned(), name: a.to_owned() });
            }
        }
        if let Some(ch) = node["children"].as_object() {
            let mut kids: Vec<_> = ch.iter().collect();
            kids.sort_by(|a, b| a.0.cmp(b.0));
            for (inst, ty) in kids {
                if let Some(ty) = ty.as_str() {
                    self.walk(ty, &format!("{prefix}{inst}/"), out);
                }
            }
        }
    }

    /// The BCM type at an instance path under a block type.
    pub fn leaf(&self, btype: &str, path: &str) -> Option<String> {
        let mut t = btype.to_owned();
        for p in path.split('/') {
            t = self.types[&t]["children"][p].as_str()?.to_owned();
        }
        Some(t)
    }

    pub fn has_custom(&self, t: &str, attr: &str) -> bool {
        self.types[t]["custom"].as_array().is_some_and(|a| a.iter().any(|x| x == attr))
    }

    pub fn settings(&self, a: &Attr) -> &[Setting] {
        self.settings.get(&(a.leaf.clone(), a.name.clone())).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn direct_bits(&self, btype: &str, path: &str) -> Option<&[String]> {
        self.direct.get(&(btype.to_owned(), path.to_owned())).map(Vec::as_slice)
    }
}

pub struct Db {
    pub dir: PathBuf,
    pub bcm: Bcm,
    maps: HashMap<String, BlockMap>,
    attrs: HashMap<String, HashMap<String, Attr>>,
}

impl Db {
    pub fn open(dir: &str) -> Result<Db> {
        let dir = PathBuf::from(dir);
        Ok(Db { bcm: Bcm::load(&dir)?, dir, maps: HashMap::new(), attrs: HashMap::new() })
    }

    /// Load a type's block map (once).
    pub fn load(&mut self, btype: &str) -> Result<()> {
        if !self.maps.contains_key(btype) {
            let m = BlockMap::load(&self.dir.join("bits").join(btype))?;
            self.maps.insert(btype.to_owned(), m);
        }
        Ok(())
    }

    /// A block map that `load` has read.
    pub fn map(&self, btype: &str) -> &BlockMap {
        &self.maps[btype]
    }

    /// A CUSTOM attribute of a block type by path.
    pub fn attribute(&mut self, btype: &str, path: &str) -> Option<Attr> {
        if !self.attrs.contains_key(btype) {
            let all = self.bcm.attributes(btype).into_iter().map(|a| (a.path.clone(), a)).collect();
            self.attrs.insert(btype.to_owned(), all);
        }
        self.attrs[btype].get(path).cloned()
    }

    /// The block type of a `<type>_r<y>_c<x>` uid, trying only types whose
    /// name is a prefix of it (no full scan of the big periphery maps).
    pub fn type_by_prefix(&mut self, uid: &str) -> Option<String> {
        let (t, _) = uid.rsplit_once("_r")?;
        let path = self.dir.join("bits").join(format!("{t}.anchors.tsv"));
        if !path.exists() {
            return None;
        }
        self.load(t).ok()?;
        self.map(t).has(uid).then(|| t.to_owned())
    }

    /// The instance of a block family (type-name prefix) at fitter X/Y:
    /// (uid, type). Some families have several types by row.
    pub fn uid_at(&mut self, family: &str, x: u32, y: u32) -> Option<(String, String)> {
        let mut types: Vec<String> = std::fs::read_dir(self.dir.join("bits"))
            .ok()?
            .filter_map(|e| e.ok()?.file_name().into_string().ok()?.strip_suffix(".anchors.tsv").map(str::to_owned))
            .filter(|t| t.starts_with(family))
            .collect();
        types.sort();
        for t in types {
            let uid = format!("{t}_r{y}_c{x}");
            self.load(&t).ok()?;
            if self.map(&t).has(&uid) {
                return Some((uid, t));
            }
        }
        None
    }

    /// Every block type with a bit map.
    pub fn types(&self) -> Result<Vec<String>> {
        let mut types: Vec<String> = std::fs::read_dir(self.dir.join("bits"))?
            .filter_map(|e| e.ok()?.file_name().into_string().ok()?.strip_suffix(".anchors.tsv").map(str::to_owned))
            .collect();
        types.sort();
        Ok(types)
    }

    /// The block type of an instance uid: `<type>_r<y>_c<x>` or a bare name.
    pub fn type_of(&mut self, uid: &str) -> Result<String> {
        let bits = self.dir.join("bits");
        let mut types: Vec<String> = std::fs::read_dir(&bits)?
            .filter_map(|e| e.ok()?.file_name().into_string().ok()?.strip_suffix(".anchors.tsv").map(str::to_owned))
            .collect();
        types.sort_by_key(|t| std::cmp::Reverse(t.len()));
        // Prefix matches first, then everything (hviowr's uids are hvio_*).
        let (pre, rest): (Vec<_>, Vec<_>) = types.into_iter().partition(|t| uid.starts_with(t.as_str()));
        for t in pre.into_iter().chain(rest) {
            self.load(&t)?;
            if self.map(&t).has(uid) {
                return Ok(t);
            }
        }
        bail!("no block map knows {uid}")
    }
}
