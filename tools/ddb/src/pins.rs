//! Package balls -> die pads -> I/O locations, from `ddb_<die><pkg>_pkg.ddb`
//! (`DEV_PKG_INFO_BODY`) and `ddb_<padname>_pad.ddb` (`DEV_PAD_INFO_BODY`),
//! both decoded with the schemas in `schema/` (checked against the trace).
//! Which info-body slot means what is inferred; see the README.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

use crate::container::Ddb;
use crate::pdb::parse_footer;
use crate::reader::{Decoded, Object, Reader, Val};
use crate::schema::Schema;

pub struct Pin {
    pub id: i64,
    /// `m_string_data`, every slot (empty = "").
    pub strings: Vec<String>,
    pub pad_ids: Vec<i64>,
}

pub struct Pad {
    pub id: i64,
    pub strings: Vec<String>,
    pub ints: Vec<i64>,
    pub io_standards: Vec<String>,
}

fn strings(o: &Object) -> Vec<String> {
    o.get("m_string_data").map(|v| v.list().iter().map(|s| s.str().unwrap_or("").to_string()).collect()).unwrap_or_default()
}
fn ints(o: &Object, name: &str) -> Vec<i64> {
    o.get(name).map(|v| v.list().iter().filter_map(Val::int).collect()).unwrap_or_default()
}

pub fn decode<'s>(path: &Path, schema: &'s Schema) -> Result<Decoded<'s>> {
    let d = Ddb::open(path).with_context(|| format!("opening {}", path.display()))?;
    let footer = parse_footer(&d.footer)?;
    Reader::new(&d.body, &footer, schema, None).decode(&footer.root_type_name).with_context(|| format!("decoding {}", path.display()))
}

pub fn pins(pkg: &Decoded) -> Result<Vec<Pin>> {
    pkg.root
        .get("m_pin_list")
        .context("no m_pin_list")?
        .list()
        .iter()
        .map(|p| {
            let o = pkg.deref("DEV_PIN", p.ptr()).context("dangling pin pointer")?;
            Ok(Pin { id: o.get("m_id").and_then(Val::int).unwrap_or(-1), strings: strings(o), pad_ids: ints(o, "m_pad_id_list") })
        })
        .collect()
}

pub fn pads(pad: &Decoded) -> Result<Vec<Pad>> {
    // I/O standard names: DEV_IO_STANDARD_DESC -> m_body -> m_string_data[0]
    let std_name = |r| -> Option<String> {
        let desc = pad.deref("DEV_IO_STANDARD_DESC", r)?;
        let body = pad.deref("DEV_IO_STANDARD_DESC_BODY", desc.get("m_body")?.ptr())?;
        Some(strings(body).first()?.clone())
    };
    pad.root
        .get("m_pad_list")
        .context("no m_pad_list")?
        .list()
        .iter()
        .map(|p| {
            let o = pad.deref("DEV_PAD", p.ptr()).context("dangling pad pointer")?;
            let b = pad.deref("DEV_PAD_BODY", o.get("m_body").and_then(Val::ptr)).context("pad without body")?;
            Ok(Pad {
                id: b.get("m_id").and_then(Val::int).unwrap_or(-1),
                strings: strings(b),
                ints: ints(b, "m_int_data"),
                io_standards: b.get("m_io_standards").map(|v| v.list().iter().filter_map(|s| std_name(s.ptr())).collect()).unwrap_or_default(),
            })
        })
        .collect()
}

/// CSV: one row per ball; pad columns empty for balls with no pad.
pub fn write_csv(out: &mut impl std::io::Write, pins: &[Pin], pads: &[Pad]) -> Result<()> {
    let by_id: HashMap<i64, &Pad> = pads.iter().map(|p| (p.id, p)).collect();
    let ns = pins.iter().map(|p| p.strings.len()).max().unwrap_or(0);
    let np = pads.iter().map(|p| p.strings.len()).max().unwrap_or(0);
    let ni = pads.iter().map(|p| p.ints.len()).max().unwrap_or(0);
    let mut hdr = vec!["pin_id".to_string()];
    hdr.extend((0..ns).map(|i| format!("pin_s{i}")));
    hdr.push("pad_id".into());
    hdr.extend((0..np).map(|i| format!("pad_s{i}")));
    hdr.extend((0..ni).map(|i| format!("pad_i{i}")));
    hdr.push("io_standards".into());
    writeln!(out, "{}", hdr.join(","))?;
    let q = |s: &str| if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    for pin in pins {
        let pads: Vec<Option<&Pad>> = if pin.pad_ids.is_empty() { vec![None] } else { pin.pad_ids.iter().map(|i| by_id.get(i).copied()).collect() };
        for pad in pads {
            let mut row = vec![pin.id.to_string()];
            row.extend((0..ns).map(|i| q(pin.strings.get(i).map(String::as_str).unwrap_or(""))));
            match pad {
                Some(p) => {
                    row.push(p.id.to_string());
                    row.extend((0..np).map(|i| q(p.strings.get(i).map(String::as_str).unwrap_or(""))));
                    row.extend((0..ni).map(|i| p.ints.get(i).map(|v| v.to_string()).unwrap_or_default()));
                    row.push(q(&p.io_standards.join(" ")));
                }
                None => row.extend(std::iter::repeat(String::new()).take(np + ni + 2)),
            }
            writeln!(out, "{}", row.join(","))?;
        }
    }
    Ok(())
}
