//! DSP blocks as multipliers (README (z11)): the four modes Quartus' dq2
//! image shows (db/sm4revb/dsp/modes.tsv) and the block's pins in them
//! [obs: `a5pnr drivers` on dq2's decode, every operand and product bit of
//! five blocks]. `MP_MAC_RE` pin 0 is the clock. An 18x18 product takes
//! operand A on pins 335 + i and B on 353 + j, and a signed one A's sign bit
//! on pin 280 as well (A is the 19-bit operand of the 18x19 multiplier); a
//! 27x27 product takes A on 353 + i (i < 9) or 362 + i, B on 335 + i (i < 9)
//! or 380 + i. Product bit k leaves on pin 134 + k (k < 38) or 60 + k, from
//! the block's output register.

use std::collections::BTreeMap;

use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mode {
    pub wide: bool,
    pub signed: bool,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match (self.wide, self.signed) {
            (false, false) => "mul18u",
            (false, true) => "mul18s",
            (true, false) => "mul27u",
            (true, true) => "mul27s",
        }
    }
    pub fn width(self) -> usize {
        if self.wide { 27 } else { 18 }
    }
}

pub const MODES: [Mode; 4] = [
    Mode { wide: false, signed: false },
    Mode { wide: false, signed: true },
    Mode { wide: true, signed: false },
    Mode { wide: true, signed: true },
];

pub const CLK: u16 = 0;
/// A signed 18x18 product's copy of A's sign bit.
pub const A_SIGN: u16 = 280;

pub fn a_pin(m: Mode, i: usize) -> u16 {
    match (m.wide, i) {
        (false, i) => 335 + i as u16,
        (true, i) if i < 9 => 353 + i as u16,
        (true, i) => 362 + i as u16,
    }
}

pub fn b_pin(m: Mode, i: usize) -> u16 {
    match (m.wide, i) {
        (false, i) => 353 + i as u16,
        (true, i) if i < 9 => 335 + i as u16,
        (true, i) => 380 + i as u16,
    }
}

pub fn p_pin(k: usize) -> u16 {
    if k < 38 { 134 + k as u16 } else { 60 + k as u16 }
}

/// The input pin a bit of the `dsp_proginv_l`/`_h` DIRECT masks inverts,
/// relative to the mode's baseline mask (db/sm4revb/dsp/modes.tsv) [obs,
/// dq2: Quartus keeps the LFSR's bit 0 as an inverted flop (it starts at 1)
/// and its mask differs from the mode's in exactly the bit of the pin that
/// reads it: l[36] for pin 353, l[18] for 335, h[27] for 398; the signed and
/// unsigned 27x27 modes share their baseline]. The baselines set the bits of
/// the pins a mode leaves unrouted, and in 27x27 mode also h[0..17], pins
/// 371..388, which carry A's bits 9..26 [inf: those pins are inverted on
/// their way in; an unrouted pin reads 1]. The pins of h[36..53] are [unk].
pub fn proginv_pin(high: bool, k: usize) -> Option<u16> {
    match (high, k) {
        (false, k) if k < 18 => Some(263 + k as u16),
        (false, k) => Some(317 + k as u16),
        (true, k) if k < 36 => Some(371 + k as u16),
        (true, _) => None,
    }
}

pub fn is_proginv(attr: &str) -> bool {
    attr.ends_with("/dsp_proginv_l") || attr.ends_with("/dsp_proginv_h")
}

/// Each mode's settings: (attribute, value) for attr and DIRECT lines alike.
pub fn modes(db: &str) -> Result<BTreeMap<Mode, Vec<(String, String, String)>>> {
    let mut out: BTreeMap<Mode, Vec<(String, String, String)>> = BTreeMap::new();
    for l in std::fs::read_to_string(format!("{db}/dsp/modes.tsv")).context("reading dsp/modes.tsv")?.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() != 4 || l.starts_with('#') || f[0] == "mode" {
            continue;
        }
        let m = *MODES.iter().find(|m| m.name() == f[0]).with_context(|| format!("dsp/modes.tsv: mode {}", f[0]))?;
        out.entry(m).or_default().push((f[1].to_owned(), f[2].to_owned(), f[3].to_owned()));
    }
    Ok(out)
}
