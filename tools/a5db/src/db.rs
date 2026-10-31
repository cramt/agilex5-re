//! The sm4revb database under db/sm4revb: image layout, per-block-type bit
//! maps (one relative layout plus an anchor per instance), the BCM type tree
//! and CUSTOM settings, and DIRECT attributes' bit lists.

use std::collections::HashMap;
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

fn tsv(path: &Path) -> Result<Vec<Vec<String>>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
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
}

impl Db {
    pub fn open(dir: &str) -> Result<Db> {
        let dir = PathBuf::from(dir);
        Ok(Db { bcm: Bcm::load(&dir)?, dir, maps: HashMap::new() })
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
