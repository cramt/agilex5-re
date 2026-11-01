//! The placed and routed design as an a5db design file (README (q), (w),
//! (y); docs/toolchain.md): pins, routing edges with bits, and per block the
//! attributes and DIRECT values that differ from the idle state, so that
//! `a5db extract` of the image gives back exactly these lines.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use anyhow::Result;

use crate::netlist::{Cell, Netlist, Sig};
use crate::pack::{Alm, Ctl, Logic, Packed, Slot, Src, LUT_IN};
use crate::place::{Lab, Placement};

/// LAB_RE pins of a LAB's control lines (README (y), ct): clock 0, enables
/// CLKEN0 and CLKEN1, the synchronous clear, and CLR1 (`regclr`'s idle
/// setting; the routing tile's `iss/xclrb` mux and inverter).
pub const LAB_CLK: u32 = 83;
pub const LAB_CE: [u32; 2] = [80, 81];
pub const LAB_SCLR: u32 = 82;
pub const LAB_ACLR: u32 = 86;
/// The routing-tile hop into LAB_ACLR whose inverter makes a clear active-low.
pub const ACLR_RPI: u32 = 5;

/// M20K settings for the 512 x 32 simple dual-port use: the 20 `xdigicore`
/// delay and enable settings mm's and mq's blocks differ from idle in
/// (README (w)); with the output register on, the last two (bypasses) stay
/// at their idle value (mq's second block).
pub const M20K_SDP: [(&str, &str); 20] = [
    ("m20k_nbl_delay", "NBL_DLY1"), ("m20k_float_rdbl_delay", "FLOAT_RDBL_DLY0"), ("m20k_float_rdbl_en", "FLOAT_RDBL_OFF"),
    ("m20k_hs_lp_en", "HIGH_SPEED_EN"), ("m20k_keeper_option", "NKEEPER_EN"), ("m20k_core_pwrmx_ctl", "VCCHG_SEL_OFF"),
    ("m20k_pwrdown", "PWRUP"), ("m20k_rclk0_delay", "RCLK0_DLY2"), ("m20k_rclkctl_delay", "RCLKCTL_DLY3"),
    ("m20k_rclkd_delay", "RCLKD_DLY3"), ("m20k_rclklat_rst", "RCLKLAT_RST_ON"), ("m20k_slow_wclk0_delay", "SLOW_WCLK0_DLY2"),
    ("m20k_tdm_delay", "TDM_DLY1"), ("m20k_wclk0_delay", "WCLK0_DLY3"), ("m20k_wclkctl_delay", "WCLKCTL_DLY0"),
    ("m20k_tdm_phase0_delay", "TDM_PHASE0_DLY2"), ("m20k_wclkd_delay", "WCLKD_DLY2"), ("m20k_wclklat_rst", "WCLKLAT_RST_ON"),
    ("m20k_b_outreg_byp", "B_OUT_REG_OFF"), ("m20k_a_outreg_byp", "A_OUT_REG_OFF"),
];

pub fn alm_path(k: u8) -> String {
    if k < 5 {
        format!("xtophalf/xale{k}")
    } else {
        format!("xbothalf/xale{}", k - 5)
    }
}

/// asm2's DIRECT lut_mask (MSB first) for a 6-LUT whose WYSIWYG mask is w
/// (bit i: index i = {F, E, D, C, B, A}): stored[j] = ¬w[j ^ 31] (README (h)).
pub fn lut_stored(w: u64) -> String {
    (0..64).rev().map(|j| if w >> (j ^ 31) & 1 == 1 { '0' } else { '1' }).collect()
}

/// An arithmetic ALM's stored mask: half h's p at bits 32h..32h+15 and g at
/// 32h+16..32h+31, as the WYSIWYG masks are (README (w)).
pub fn arith_stored(pg: [(u16, u16); 2]) -> String {
    let mut v = 0u64;
    for (h, (p, g)) in pg.iter().enumerate() {
        v |= (*p as u64) << (32 * h) | (*g as u64) << (32 * h + 16);
    }
    (0..64).rev().map(|j| if v >> j & 1 == 1 { '1' } else { '0' }).collect()
}

/// A LUT over its inputs (bit k of the index is input k) as a 64-entry
/// WYSIWYG mask over A..F, input k on position pins[k].
pub fn widen(mask: u64, pins: &[u8]) -> u64 {
    (0..64u64).fold(0u64, |acc, j| {
        let idx = pins.iter().enumerate().fold(0usize, |x, (k, &p)| x | ((j >> p & 1) as usize) << k);
        acc | (mask >> idx & 1) << j
    })
}

/// A LAB's control lines after placement.
#[derive(Default, Debug, Clone)]
pub struct LabLines {
    pub clock: Option<u32>,
    pub ce: Vec<u32>,
    pub sclr: Option<u32>,
    pub aclr: Option<(u32, bool)>,
}

pub fn lab_lines(p: &Packed, nl: &Netlist, pl: &Placement) -> BTreeMap<usize, LabLines> {
    let mut out: BTreeMap<usize, LabLines> = BTreeMap::new();
    for (a, alm) in p.alms.iter().enumerate() {
        let l = out.entry(pl.at[a].0).or_default();
        for c in alm.ctls(nl) {
            match c {
                Ctl::Clock(n) => l.clock = Some(n),
                Ctl::Ce(n) => {
                    if !l.ce.contains(&n) {
                        l.ce.push(n);
                    }
                }
                Ctl::Sclr(n) => l.sclr = Some(n),
                Ctl::Aclr(Some(x)) => l.aclr = Some(x),
                Ctl::Aclr(None) => {}
            }
        }
    }
    for l in out.values_mut() {
        l.ce.sort();
    }
    out
}

/// The LAB_RE pin a control end lands on.
pub fn ctl_pin(lines: &LabLines, c: Ctl) -> u32 {
    match c {
        Ctl::Clock(_) => LAB_CLK,
        Ctl::Ce(n) => LAB_CE[lines.ce.iter().position(|&m| m == n).expect("enable on its LAB")],
        Ctl::Sclr(_) => LAB_SCLR,
        Ctl::Aclr(_) => LAB_ACLR,
    }
}

pub fn lab_uid(labs: &[Lab], l: usize) -> String {
    format!("cw_la_lab_r{}_c{}", labs[l].y, labs[l].x)
}

/// Attribute and DIRECT lines for the logic: ALMs, LABs, M20Ks.
pub fn blocks(nl: &Netlist, p: &Packed, labs: &[Lab], pl: &Placement, lines: &BTreeMap<usize, LabLines>) -> (Vec<(String, String, String)>, Vec<(String, String, String)>) {
    let mut attrs: Vec<(String, String, String)> = Vec::new();
    let mut directs: Vec<(String, String, String)> = Vec::new();
    let mut ff_labs = BTreeSet::new();
    for (a, alm) in p.alms.iter().enumerate() {
        let (l, k) = pl.at[a];
        let uid = lab_uid(labs, l);
        let pa = alm_path(k);
        let mut set = |path: String, v: &str| attrs.push((uid.clone(), path, v.to_owned()));
        let ll = &lines[&l];
        match &alm.logic {
            Logic::Lut { cell, pins } => {
                let Cell::Lut { mask, .. } = &nl.cells[*cell] else { unreachable!() };
                // A constant-0 LUT's stored mask is all ones: the idle value,
                // which an image doesn't show.
                let stored = lut_stored(widen(*mask, pins));
                if stored.contains('0') {
                    directs.push((uid.clone(), format!("{pa}/xcw_la_le_lut/lut_mask"), stored));
                }
            }
            Logic::Arith { halves } => {
                let mut pg = [(0u16, 0u16); 2];
                for (h, c) in halves.iter().enumerate() {
                    if let Some(c) = c {
                        let Cell::Arith { p: pm, g: gm, .. } = nl.cells[*c] else { unreachable!() };
                        pg[h] = (pm, gm);
                    }
                }
                set(format!("{pa}/xcw_la_le_lut/adder_mode"), "ENABLE_ADDER");
                set(format!("{pa}/xce_la_le_adder/adder_or_packed_mode"), "ENABLE_ADDER_OR_PACKED");
                directs.push((uid.clone(), format!("{pa}/xcw_la_le_lut/lut_mask"), arith_stored(pg)));
            }
            Logic::None => {}
        }
        let mut ce_line = 0;
        for h in 0..2usize {
            let r0 = format!("{pa}/xcw_la_le_reg{h}");
            let mut half_ff = None;
            for r in 0..2usize {
                let Some(slot) = alm.slots[2 * h + r] else { continue };
                let base = format!("{r0}/xreg{r}");
                let src = match slot {
                    Slot::Ff { src, .. } | Slot::Wire { src } => src,
                };
                // ARITH_SUMOUT is reg_input's idle setting (both bits 0).
                match src {
                    Src::Lut => set(format!("{base}/reg_input"), "LUTOUT_6LUT"),
                    Src::Leim => set(format!("{base}/reg_input"), "LEIM"),
                    Src::Sum => {}
                }
                set(format!("{base}/leout"), "LEOUT");
                match slot {
                    Slot::Ff { ff, .. } => {
                        let Cell::Ff(f) = &nl.cells[ff] else { unreachable!() };
                        set(format!("{base}/register_comb"), "DFF_LATCH");
                        if let Some(ce) = f.ce {
                            set(format!("{base}/ce_sel"), "CE");
                            ce_line = ll.ce.iter().position(|&m| m == ce).unwrap();
                        }
                        if f.sclr.is_some() {
                            set(format!("{base}/sclr_sel"), "SCLR");
                        }
                        half_ff = Some(f.clone());
                    }
                    Slot::Wire { .. } => set(format!("{base}/register_comb"), "WIRE_MODE"),
                }
            }
            if half_ff.is_some() {
                set(format!("{r0}/xlabclk_mux/regclk"), "DFF_LABCLK0");
                ff_labs.insert(l);
            }
            // A half whose flops use the LAB's clear stays on CLR1 (idle);
            // everything else is on CLR2, left inactive (Quartus, ct).
            if half_ff.as_ref().is_none_or(|f| f.aclr.is_none()) {
                set(format!("{r0}/xreg_aclr/regclr"), "CLR2");
            }
        }
        if ce_line == 1 {
            set(format!("{pa}/xcw_la_le_regctrl/xce_la_le_regctrl_mux/regclk_ens"), "CLKEN1");
        }
        set(format!("{pa}/xcw_la_le_regctrl/xcw_la_le_regctrl_reg/ce_sel"), "CE");
        set(format!("{pa}/xcw_la_le_regctrl/xcw_la_le_regctrl_reg/clr_sel"), "CLR");
    }
    for &l in &ff_labs {
        attrs.push((lab_uid(labs, l), "xsecmux/xcw_la_lcb_clkgen/xlabclk0/smxclk_tie_off".into(), "DISABLE_TIEOFF".into()));
    }
    // Carry between the LAB's halves and into the LAB below (README (y)).
    let lab_at: std::collections::HashMap<(u32, u32), usize> = labs.iter().enumerate().map(|(i, l)| ((l.x, l.y), i)).collect();
    for c in &p.chains {
        for w in c.windows(2) {
            let ((l0, k0), (l1, k1)) = (pl.at[w[0]], pl.at[w[1]]);
            if l0 == l1 && k0 == 4 && k1 == 5 {
                attrs.push((lab_uid(labs, l0), "xsecmux/xcw_la_lcb_adder/secmux_bto".into(), "DISABLE_BTO".into()));
            }
            if k0 == 9 && k1 == 0 && lab_at.get(&(labs[l0].x, labs[l0].y - 1)) == Some(&l1) {
                attrs.push((lab_uid(labs, l0), "xsecmux/xcw_la_lcb_adder/hipi_clken".into(), "COMB_PATH".into()));
            }
        }
    }
    // Active-low clears: the routing tile's inverter (README (y), ct).
    for (&l, ll) in lines {
        if let Some((_, true)) = ll.aclr {
            attrs.push((format!("cw_rt_lab_base_r{}_c{}", labs[l].y, labs[l].x), "iss/xclrb_rpi/sel".into(), "INV".into()));
        }
    }
    for (m, &(x, y)) in pl.m20k_at.iter().enumerate() {
        let Cell::M20k(c) = &nl.cells[p.m20ks[m]] else { unreachable!() };
        let uid = format!("cw_mm_m20k_r{y}_c{x}");
        for (a, v) in M20K_SDP {
            if c.outreg && a.ends_with("outreg_byp") {
                continue;
            }
            attrs.push((uid.clone(), format!("xdigicore/{a}"), v.to_string()));
        }
        if c.we == Sig::Const(false) {
            attrs.push((uid.clone(), "xsecmux/xwe0/m20k_invsel".into(), "INV_LEIM_SEL".into()));
        }
        // Contents: word a is m20k_eram_add<a>, 40 bits in the lane layout,
        // MSB first (mq: 512 of 512 words). Zero words are the idle state.
        for (a, &w) in c.init.iter().enumerate() {
            if w == 0 {
                continue;
            }
            let v: u64 = (0..32).fold(0, |v, k| v | ((w >> k & 1) as u64) << crate::pack::m20k_pos(k));
            directs.push((uid.clone(), format!("xdigicore/xdigicorepg/xcore/xary/m20k_eram_add{a}"), format!("{v:040b}")));
        }
    }
    attrs.sort();
    attrs.dedup();
    (attrs, directs)
}

/// The ALM input pin for LUT position p.
pub fn lut_pin(p: u8) -> u8 {
    LUT_IN[p as usize]
}

/// Which netlist register each image flop holds (for simulation only: the
/// readback check lines up RTL state with the image's flops by it).
pub fn ffmap(nl: &Netlist, p: &Packed, labs: &[Lab], pl: &Placement) -> Result<String> {
    let mut map = String::new();
    for (a, alm) in p.alms.iter().enumerate() {
        let l = labs[pl.at[a].0];
        let k = pl.at[a].1 as u32;
        for s in 0..4u32 {
            if let Some(Slot::Ff { ff, .. }) = alm.slots[s as usize] {
                let Cell::Ff(f) = &nl.cells[ff] else { unreachable!() };
                // A flop yosys keeps inverted (dfflegalize's flip_bits for an
                // init value of 1) has no name of its own: the register is
                // the inverter's output, written `!name`.
                let inverted = || {
                    nl.cells.iter().find_map(|c| match c {
                        Cell::Lut { ins, mask: 1, out } if ins.as_slice() == [f.q] => nl.aliases.get(out),
                        _ => None,
                    })
                };
                let names = match nl.aliases.get(&f.q) {
                    Some(v) => v.join("|"),
                    None => match inverted() {
                        Some(v) => v.iter().map(|n| format!("!{n}")).collect::<Vec<_>>().join("|"),
                        None => nl.nets[f.q as usize].clone(),
                    },
                };
                writeln!(map, "{names}\tl{}_{}_o{}", l.x, l.y, 89 + 4 * k + s)?;
            }
        }
    }
    let _: Option<&Alm> = None;
    Ok(map)
}
