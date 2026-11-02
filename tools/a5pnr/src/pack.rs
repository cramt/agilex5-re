//! Packing cells into ALMs (README (w), (y), (z3)).
//!
//! An ALM has eight input pins, E0, D0, C0, A, B, C1, D1, E1 (offsets 0..7
//! of its LAB_RE pins 8k..8k + 7), a 64-bit LUT mask and four register
//! slots [obs, Quartus' images ar, hx, mm, ct, mq and fr, README (z3)]:
//!
//! - the low 32 mask bits are a 5-input LUT over (A, B, C0, D0, E0), the
//!   high 32 one over (A, B, C1, D1, E1); each is a register slot input
//!   `LUTOUT_5LUT` in its own half (half 0 low, half 1 high), so an ALM
//!   holds two LUTs of up to five inputs that share A and B;
//! - `LUTOUT_6LUT` (either half) is E1 ? high(A, B, C1, D1, E0) :
//!   low(A, B, C0, D0, E0): a 6-LUT's C and D go to C1 and D1 as well,
//!   wherever the upper half reads them;
//! - in arithmetic mode half h's C and D are C0, D0 (h = 0) or C1, D1.
//!
//! Slot s is register `xcw_la_le_reg{s / 2}/xreg{s % 2}` and drives LAB_RE
//! pin 89 + 4k + s; it is a flop or a wire (WIRE_MODE, the combinational
//! output). A slot's input is its half's LUT, its half's sum, or (`reg_input`
//! LEIM) pin A for half 0 and B for half 1 [obs, hx and mm's quartus_sta
//! paths]. A flop with no logic of its own shares an ALM with unrelated
//! logic through LEIM, or through a half with no LUT whose LUT passes one
//! pin through (Quartus does both, ar and fr).
//!
//! Rules an ALM must follow (README (y)): the two slots of a half share a
//! clock and an asynchronous clear (`regclk`, `regclr` are per half); every
//! flop with an enable uses the ALM's one enable line (`regclk_ens`); only
//! the halves' xreg0 have `ce_sel` and `sclr_sel`, which act on the whole half,
//! so a half's two flops share their enable and synchronous clear (README (z3)).

use std::collections::BTreeMap;
use crate::det::{HashMap, HashSet};

use anyhow::{bail, Result};

use crate::netlist::{Aclr, Cell, Ff, Netlist, Sig};

/// A register slot's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Src {
    /// The ALM's LUT: `LUTOUT_6LUT` for a 6-LUT, else the slot's half's
    /// 5-LUT (`LUTOUT_5LUT`).
    Lut,
    /// The half's arithmetic sum (`ARITH_SUMOUT`).
    Sum,
    /// The ALM's pin A (half 0) or B (half 1) (`LEIM`).
    Leim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Ff { ff: usize, src: Src },
    Wire { src: Src },
}

/// What a half's 5-LUT computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HalfLut {
    /// A netlist LUT cell.
    Cell(usize),
    /// One net passed through, for flops with no logic of their own.
    Pass(u32),
}

/// A 5-LUT in one half: its function and, per input, the ALM pin it is on.
#[derive(Clone, Debug)]
pub struct Half {
    pub lut: HalfLut,
    pub pins: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum Logic {
    /// A 6-input LUT (`LUTOUT_6LUT`): input k at position pos[k] in A..F.
    Six { cell: usize, pos: Vec<u8> },
    /// Up to two 5-input LUTs, one per half (`LUTOUT_5LUT`); none for a
    /// flop-only ALM.
    Five { halves: [Option<Half>; 2] },
    Arith { halves: [Option<usize>; 2] },
}

#[derive(Clone, Debug)]
pub struct Alm {
    pub logic: Logic,
    pub slots: [Option<Slot>; 4],
}

/// The control signals a register uses, as the LAB has to provide them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ctl {
    Clock(u32),
    Ce(u32),
    Sclr(u32),
    /// An asynchronous clear; None is a line left inactive.
    Aclr(Option<(u32, bool)>),
}

pub const PIN_A: u8 = 3;
pub const PIN_B: u8 = 4;
/// 6-LUT position (A..F) → pin (README (n)); C and D also C1, D1.
pub const SIX_PIN: [u8; 6] = [3, 4, 2, 1, 0, 7];
pub const SIX_HI_PIN: [u8; 6] = [3, 4, 5, 6, 0, 7];
/// A half's own pins C, D, E.
pub const HALF_PINS: [[u8; 3]; 2] = [[2, 1, 0], [5, 6, 7]];
/// An arithmetic half's C and D inputs (README (w)).
pub const ARITH_IN: [[u8; 2]; 2] = [[2, 1], [5, 6]];
/// The pin a half's LEIM slots read: A, B.
pub const LEIM_PIN: [u8; 2] = [PIN_A, PIN_B];

/// Index bit of an ALM pin in half h's 5-input function: A 0, B 1, C 2,
/// D 3, E 4 (README (z3)).
pub fn half_bit(h: usize, pin: u8) -> usize {
    match pin {
        PIN_A => 0,
        PIN_B => 1,
        p => 2 + HALF_PINS[h].iter().position(|&q| q == p).expect("a pin of the half"),
    }
}

/// The pins a LUT input on `pin` of ALM `alm` may move to when the router
/// swaps inputs (README (z8)), `pin` first; just `pin` when it can't move.
/// A 6-LUT's inputs on A, B, E and F trade places (C and D are on both
/// halves' pins); a 5-LUT half's on its own C, D and E (A and B are the
/// other half's too); arithmetic inputs and the pins a LEIM flop reads stay.
pub fn swap_group(alm: &Alm, pin: u8) -> Vec<u8> {
    let leim: Vec<u8> = (0..4).filter(|&s| matches!(alm.slots[s], Some(Slot::Ff { src: Src::Leim, .. }))).map(|s| LEIM_PIN[s / 2]).collect();
    let group: Vec<u8> = match &alm.logic {
        Logic::Six { .. } if [PIN_A, PIN_B, 0, 7].contains(&pin) => vec![PIN_A, PIN_B, 0, 7],
        Logic::Five { halves } => match (0..2).find(|&h| HALF_PINS[h].contains(&pin)) {
            Some(h) if halves[h].is_some() => HALF_PINS[h].to_vec(),
            _ => vec![],
        },
        _ => vec![],
    };
    if leim.contains(&pin) {
        return vec![pin];
    }
    std::iter::once(pin).chain(group.into_iter().filter(|q| *q != pin && !leim.contains(q))).collect()
}

/// Move an ALM's LUT inputs to the pins the router chose (`moves`: old pin
/// → new pin, all within swap groups); the masks follow from the new pins.
pub fn apply_swaps(alm: &mut Alm, moves: &[(u8, u8)]) {
    let to = |q: u8| moves.iter().find(|m| m.0 == q).map_or(q, |m| m.1);
    match &mut alm.logic {
        Logic::Six { pos, .. } => {
            for p in pos.iter_mut() {
                if (*p as usize) < 6 && six_pins(*p).len() == 1 {
                    let new = to(SIX_PIN[*p as usize]);
                    *p = SIX_PIN.iter().position(|&q| q == new).expect("a 6-LUT pin") as u8;
                }
            }
        }
        Logic::Five { halves } => {
            for half in halves.iter_mut().flatten() {
                for q in half.pins.iter_mut() {
                    *q = to(*q);
                }
            }
        }
        Logic::Arith { .. } => {}
    }
}

/// The delay model's LUT input class of a pin (README (u)).
pub fn pin_letter(pin: u8) -> char {
    match pin {
        3 => 'a',
        4 => 'b',
        2 | 5 => 'c',
        1 | 6 => 'd',
        0 => 'e',
        _ => 'f',
    }
}

/// Where a net comes from or goes to, before placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum End {
    /// ALM a's register slot s (LAB_RE 89 + 4k + s).
    Out { alm: usize, slot: u8 },
    /// ALM a's input pin, as the offset within the ALM's 8 LAB_RE pins.
    In { alm: usize, pin: u8 },
    /// A LAB control line ALM a needs (clock, enable, clears).
    Ctl { alm: usize, ctl: Ctl },
    /// A top-level port.
    Port(usize),
    /// Pin of M20K m (`MEDIUM_EAB_RE` index), input or output by context.
    M20k { m: usize, pin: u16 },
    /// IOPLL k's output.
    Pll(usize),
}

/// The M20K's pins in its 512 x 32 simple dual-port use (README (w), (y)):
/// from mm's and mq's quartus_sta paths. Data bit k sits at position
/// 10 (k / 8) + k % 8 of the 40-bit word (four 10-bit byte lanes, as Quartus
/// lays it out with byte enables); position j is write-data pin 57 + j
/// (j < 20) or 40 - j and read-data pin 83 + j.
pub const M20K_CLK: u16 = 77;
pub const M20K_WE: u16 = 37;
pub const M20K_RE: u16 = 39;
pub const M20K_BE: [u16; 4] = [55, 56, 22, 21];
pub fn m20k_pos(k: usize) -> usize { 10 * (k / 8) + k % 8 }
pub fn m20k_waddr(i: usize) -> u16 { 25 + i as u16 }
pub fn m20k_raddr(i: usize) -> u16 { 52 - i as u16 }
pub fn m20k_wdata(k: usize) -> u16 { let j = m20k_pos(k) as u16; if j < 20 { 57 + j } else { 40 - j } }
pub fn m20k_rdata(k: usize) -> u16 { 83 + m20k_pos(k) as u16 }

pub struct Packed {
    pub alms: Vec<Alm>,
    /// Chains as ALM lists, in chain order.
    pub chains: Vec<Vec<usize>>,
    /// M20K cells, by netlist index.
    pub m20ks: Vec<usize>,
    /// Per net: driver and sinks.
    pub nets: BTreeMap<u32, (End, Vec<End>)>,
    /// Clock nets.
    pub clocks: Vec<u32>,
    /// IOPLL cells, by netlist index.
    pub plls: Vec<usize>,
}

/// A 6-LUT's mask over A..F (bit i: index i = {F, E, D, C, B, A}) with
/// input k at position pos[k].
pub fn widen(mask: u64, pos: &[u8]) -> u64 {
    (0..64u64).fold(0u64, |acc, j| {
        let idx = pos.iter().enumerate().fold(0usize, |x, (k, &p)| x | ((j >> p & 1) as usize) << k);
        acc | (mask >> idx & 1) << j
    })
}

/// Which pins a 6-LUT's input at position p is read on: C and D go to both
/// halves' pins (C0 and C1, D0 and D1), as Quartus routes them in every
/// 6-LUT of fr, so which half reads which is moot (README (z3)).
pub fn six_pins(p: u8) -> Vec<u8> {
    if p == 2 || p == 3 {
        vec![SIX_PIN[p as usize], SIX_HI_PIN[p as usize]]
    } else {
        vec![SIX_PIN[p as usize]]
    }
}

/// A half's 32-entry function over its pins (bit j: index j = {E, D, C,
/// B, A} of half h), from a LUT whose input k is on pins[k].
pub fn half_function(h: usize, mask: u64, pins: &[u8]) -> u32 {
    (0..32u32).fold(0u32, |acc, j| {
        let idx = pins.iter().enumerate().fold(0usize, |x, (k, &p)| x | ((j >> half_bit(h, p) & 1) as usize) << k);
        acc | ((mask >> idx & 1) as u32) << j
    })
}

/// Put two LUTs (input nets l0, l1) in the halves of one ALM: the nets on A
/// and B are read by both, every other input goes on its half's C, D, E.
/// A and B carry as few nets as possible (they are the LEIM pins). Returns
/// each LUT's pins.
pub fn fit_halves(l0: &[u32], l1: &[u32]) -> Option<(Vec<u8>, Vec<u8>)> {
    if l0.len() > 5 || l1.len() > 5 {
        return None;
    }
    let mut all: Vec<u32> = l0.to_vec();
    for n in l1 {
        if !all.contains(n) {
            all.push(*n);
        }
    }
    // Shared nets first: they save a pin on A or B.
    all.sort_by_key(|n| !(l0.contains(n) && l1.contains(n)));
    let mut cands: Vec<Vec<u32>> = vec![vec![]];
    for i in 0..all.len() {
        cands.push(vec![all[i]]);
    }
    for i in 0..all.len() {
        for j in i + 1..all.len() {
            cands.push(vec![all[i], all[j]]);
        }
    }
    for s in cands {
        let r0: Vec<u32> = l0.iter().copied().filter(|n| !s.contains(n)).collect();
        let r1: Vec<u32> = l1.iter().copied().filter(|n| !s.contains(n)).collect();
        if r0.len() > 3 || r1.len() > 3 {
            continue;
        }
        let place = |l: &[u32], r: &[u32], h: usize| -> Vec<u8> {
            l.iter()
                .map(|n| match s.iter().position(|m| m == n) {
                    Some(i) => [PIN_A, PIN_B][i],
                    // E first: the fastest of a half's pins (README (u)).
                    None => HALF_PINS[h][2 - r.iter().position(|m| m == n).unwrap()],
                })
                .collect()
        };
        return Some((place(l0, &r0, 0), place(l1, &r1, 1)));
    }
    None
}

impl Alm {
    fn empty() -> Alm {
        Alm { logic: Logic::Five { halves: [None, None] }, slots: [None; 4] }
    }

    fn ff(&self, nl: &Netlist, s: usize) -> Option<Ff> {
        match self.slots[s] {
            Some(Slot::Ff { ff, .. }) => match &nl.cells[ff] {
                Cell::Ff(f) => Some(f.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    /// The control lines this ALM's flops need.
    pub fn ctls(&self, nl: &Netlist) -> Vec<Ctl> {
        let mut out = Vec::new();
        for h in 0..2 {
            let ffs: Vec<Ff> = (2 * h..2 * h + 2).filter_map(|s| self.ff(nl, s)).collect();
            if let Some(f) = ffs.first() {
                out.push(Ctl::Clock(f.clk));
                out.push(Ctl::Aclr(f.aclr.map(|a| (a.net, a.active_low))));
            }
            for f in &ffs {
                out.extend(f.ce.map(Ctl::Ce));
                out.extend(f.sclr.map(Ctl::Sclr));
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Could flop f go into slot s (pin rules aside)?
    fn fits(&self, nl: &Netlist, s: usize, f: &Ff) -> bool {
        if self.slots[s].is_some() {
            return false;
        }
        // xreg0's ce_sel and sclr_sel serve both registers of the half
        // (README (z3)): the two flops of a half have the same enable and
        // synchronous clear.
        if let Some(o) = self.ff(nl, s ^ 1) {
            if o.ce != f.ce || o.sclr != f.sclr {
                return false;
            }
        }
        // The two slots of a half share a clock and a clear.
        let aclr = |a: Option<Aclr>| a.map(|a| (a.net, a.active_low));
        if let Some(o) = self.ff(nl, s ^ 1) {
            if o.clk != f.clk || aclr(o.aclr) != aclr(f.aclr) {
                return false;
            }
        }
        (0..4).filter_map(|t| self.ff(nl, t)).all(|o| compatible(&o, f))
    }

    /// The net on each pin, as far as the logic and LEIM flops decide it.
    pub fn pin_nets(&self, nl: &Netlist) -> [Option<u32>; 8] {
        let mut out = [None; 8];
        match &self.logic {
            Logic::Six { cell, pos } => {
                let Cell::Lut { ins, .. } = &nl.cells[*cell] else { unreachable!() };
                for (k, &p) in pos.iter().enumerate() {
                    for q in six_pins(p) {
                        out[q as usize] = Some(ins[k]);
                    }
                }
            }
            Logic::Five { halves } => {
                for half in halves.iter().flatten() {
                    for (k, &p) in half.pins.iter().enumerate() {
                        out[p as usize] = Some(match half.lut {
                            HalfLut::Cell(c) => {
                                let Cell::Lut { ins, .. } = &nl.cells[c] else { unreachable!() };
                                ins[k]
                            }
                            HalfLut::Pass(n) => n,
                        });
                    }
                }
            }
            Logic::Arith { halves } => {
                for (h, half) in halves.iter().enumerate() {
                    let Some(c) = half else { continue };
                    let Cell::Arith { c, d, .. } = &nl.cells[*c] else { unreachable!() };
                    for (k, n) in [c, d].into_iter().enumerate() {
                        if let Some(n) = n {
                            out[ARITH_IN[h][k] as usize] = Some(*n);
                        }
                    }
                }
            }
        }
        for s in 0..4 {
            if let Some(Slot::Ff { ff, src: Src::Leim }) = self.slots[s] {
                if let Cell::Ff(Ff { d: Sig::Net(d), .. }) = nl.cells[ff] {
                    out[LEIM_PIN[s / 2] as usize] = Some(d);
                }
            }
        }
        out
    }

    /// Put flop ff (D = net d) into a free slot: through LEIM when the
    /// half's LEIM pin is free or already carries d, else through a half
    /// with no LUT, which then passes d through (Quartus does both, ar).
    fn try_flop(&mut self, nl: &Netlist, ff: usize, d: u32) -> Option<usize> {
        let Cell::Ff(f) = &nl.cells[ff] else { return None };
        let pins = self.pin_nets(nl);
        for s in 0..4 {
            if self.fits(nl, s, f) && pins[LEIM_PIN[s / 2] as usize].is_none_or(|n| n == d) {
                self.slots[s] = Some(Slot::Ff { ff, src: Src::Leim });
                return Some(s);
            }
        }
        for s in 0..4 {
            let h = s / 2;
            if !self.fits(nl, s, f) {
                continue;
            }
            let Logic::Five { halves } = &mut self.logic else { return None };
            match &halves[h] {
                Some(Half { lut: HalfLut::Pass(n), .. }) if *n == d => {}
                Some(_) => continue,
                None => halves[h] = Some(Half { lut: HalfLut::Pass(d), pins: vec![HALF_PINS[h][2]] }),
            }
            self.slots[s] = Some(Slot::Ff { ff, src: Src::Lut });
            return Some(s);
        }
        None
    }
}

/// Can two flops share an ALM (in different halves): one clock, one enable
/// line, and what the LAB has only one of (a synchronous clear, a routed
/// clear: README (y))?
fn compatible(o: &Ff, f: &Ff) -> bool {
    let aclr = |a: Option<Aclr>| a.map(|a| (a.net, a.active_low));
    !(f.ce.is_some_and(|c| o.ce.is_some_and(|d| d != c))
        || o.clk != f.clk
        || f.sclr.is_some_and(|c| o.sclr.is_some_and(|d| d != c))
        || matches!((aclr(o.aclr), aclr(f.aclr)), (Some(a), Some(b)) if a != b))
}

/// LUT inputs on 6-LUT positions, the fastest first (F, E, … in the delay
/// model, README (u)).
fn six_positions(n: usize) -> Vec<u8> {
    const ORDER: [u8; 6] = [5, 4, 3, 2, 1, 0];
    ORDER[..n].to_vec()
}

/// How a5pnr packs LUTs (`--pack`): `pairs` puts two LUTs of up to five
/// inputs into one ALM wherever their inputs fit, related ones first;
/// `related` only pairs LUTs that share an input or a connection; `single`
/// is phase 7's one LUT per ALM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackMode {
    Pairs,
    Related,
    Single,
}

pub fn pack(nl: &Netlist, mode: PackMode) -> Result<Packed> {
    let mut driver: HashMap<u32, usize> = HashMap::default();
    let mut loads: HashMap<u32, Vec<(usize, &'static str)>> = HashMap::default();
    for (i, c) in nl.cells.iter().enumerate() {
        let mut add = |s: Sig, port: &'static str| {
            if let Sig::Net(n) = s {
                loads.entry(n).or_default().push((i, port));
            }
        };
        match c {
            Cell::Lut { ins, out, .. } => {
                driver.insert(*out, i);
                for &n in ins {
                    add(Sig::Net(n), "lut");
                }
            }
            Cell::Arith { c, d, ci, so, co, .. } => {
                for n in [c, d].into_iter().flatten() {
                    add(Sig::Net(*n), "arith");
                }
                add(*ci, "ci");
                for n in [so, co].into_iter().flatten() {
                    driver.insert(*n, i);
                }
            }
            Cell::Ff(f) => {
                if f.sclr_first && f.ce.is_some() {
                    bail!("a flop whose synchronous clear overrides its enable: the LAB's enable gates the clear (README (y)); synth.sh legalises these");
                }
                driver.insert(f.q, i);
                add(f.d, "d");
                add(Sig::Net(f.clk), "clk");
                for n in [f.ce, f.sclr, f.aclr.map(|a| a.net)].into_iter().flatten() {
                    add(Sig::Net(n), "ctl");
                }
            }
            Cell::Pll { out, .. } => {
                driver.insert(*out, i);
            }
            Cell::M20k(m) => {
                for s in m.waddr.iter().chain(&m.wdata).chain(&m.raddr).chain(&m.be).chain([&m.we, &m.re]) {
                    add(*s, "m20k");
                }
                add(Sig::Net(m.clk), "clk");
                for n in m.rdata.iter().flatten() {
                    driver.insert(*n, i);
                }
            }
        }
    }
    for p in nl.ports.iter().filter(|p| !p.input) {
        if let Sig::Net(n) = p.net {
            loads.entry(n).or_default().push((usize::MAX, "port"));
        }
    }
    let mut clocks: Vec<u32> = nl
        .cells
        .iter()
        .filter_map(|c| match c {
            Cell::Ff(f) => Some(f.clk),
            Cell::M20k(m) => Some(m.clk),
            _ => None,
        })
        .collect();
    clocks.sort();
    clocks.dedup();
    // Is the output used other than by flop ff's D?
    let comb_used = |net: u32, ff: Option<usize>| -> bool {
        loads.get(&net).is_some_and(|l| l.iter().any(|&(c, port)| !(Some(c) == ff && port == "d")))
    };
    let mut placed: HashSet<usize> = Default::default();
    let mut alms: Vec<Alm> = Vec::new();
    let mut ffs_on: HashMap<u32, Vec<usize>> = HashMap::default();
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Ff(Ff { d: Sig::Net(d), .. }) = c {
            ffs_on.entry(*d).or_default().push(i);
        }
    }
    let ff_of = |i: usize| -> &Ff {
        let Cell::Ff(f) = &nl.cells[i] else { unreachable!() };
        f
    };
    // Carry chains, from their heads.
    let mut next: HashMap<usize, usize> = HashMap::default();
    let mut has_prev = HashSet::default();
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Arith { co: Some(co), .. } = c {
            for &(l, port) in loads.get(co).map(|v| v.as_slice()).unwrap_or(&[]) {
                if port != "ci" {
                    bail!("a carry out feeds something other than a carry in");
                }
                if next.insert(i, l).is_some() {
                    bail!("a carry out feeds more than one carry in");
                }
                has_prev.insert(l);
            }
        }
    }
    let mut chains = Vec::new();
    for (i, c) in nl.cells.iter().enumerate() {
        if !matches!(c, Cell::Arith { .. }) || has_prev.contains(&i) {
            continue;
        }
        let mut cells = vec![i];
        while let Some(&n) = next.get(cells.last().unwrap()) {
            cells.push(n);
        }
        // The hardware carry into a chain's first half is 0 (README (y)), so
        // a start cell whose carry out is a constant 0 (p = g = 0, no sum)
        // isn't needed.
        if let Cell::Arith { p: 0, g: 0, so, .. } = nl.cells[cells[0]] {
            if cells.len() > 1 && so.is_none_or(|n| !loads.contains_key(&n)) {
                cells.remove(0);
            }
        }
        let mut chain = Vec::new();
        for pair in cells.chunks(2) {
            let mut alm = Alm { logic: Logic::Arith { halves: [pair.first().copied(), pair.get(1).copied()] }, slots: [None; 4] };
            for (h, &ci) in pair.iter().enumerate() {
                let Cell::Arith { so: Some(so), .. } = nl.cells[ci] else { continue };
                let ff = ffs_on.get(&so).and_then(|v| v.iter().copied().find(|f| !placed.contains(f)));
                let ff = ff.filter(|&f| alm.fits(nl, 2 * h, ff_of(f)));
                if let Some(f) = ff {
                    alm.slots[2 * h] = Some(Slot::Ff { ff: f, src: Src::Sum });
                    placed.insert(f);
                }
                if comb_used(so, ff) {
                    alm.slots[2 * h + 1] = Some(Slot::Wire { src: Src::Sum });
                }
            }
            chain.push(alms.len());
            alms.push(alm);
        }
        chains.push(chain);
    }
    // Each LUT's own flop: the first one on its output not taken yet.
    let mut own_ff: HashMap<usize, usize> = HashMap::default();
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Lut { out, .. } = c {
            if let Some(f) = ffs_on.get(out).and_then(|v| v.iter().copied().find(|f| !placed.contains(f))) {
                own_ff.insert(i, f);
                placed.insert(f);
            }
        }
    }
    // A LUT's slots in half h: its flop in xreg0 (which has the enable and
    // clear selects), its combinational output in xreg1.
    let lut_slots = |alm: &mut Alm, i: usize, out: u32, h: usize| {
        let ff = own_ff.get(&i).copied();
        if let Some(f) = ff {
            alm.slots[2 * h] = Some(Slot::Ff { ff: f, src: Src::Lut });
        }
        if comb_used(out, ff) {
            alm.slots[2 * h + 1] = Some(Slot::Wire { src: Src::Lut });
        }
    };
    let ins_of = |i: usize| -> &[u32] {
        let Cell::Lut { ins, .. } = &nl.cells[i] else { unreachable!() };
        ins
    };
    let out_of = |i: usize| -> u32 {
        let Cell::Lut { out, .. } = &nl.cells[i] else { unreachable!() };
        *out
    };
    let small: Vec<usize> = nl
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, Cell::Lut { ins, .. } if ins.len() <= 5 || mode == PackMode::Single))
        .map(|(i, _)| i)
        .collect();
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Lut { ins, out, .. } = c {
            if ins.len() == 6 || mode == PackMode::Single {
                let mut alm = Alm { logic: Logic::Six { cell: i, pos: six_positions(ins.len()) }, slots: [None; 4] };
                lut_slots(&mut alm, i, *out, 0);
                alms.push(alm);
            }
        }
    }
    if mode != PackMode::Single {
        // Pairs: each LUT (most inputs first) with the unpaired LUT that
        // shares the most inputs or a connection with it, else (`pairs`)
        // with any whose inputs still fit, the largest that does.
        let mut order = small.clone();
        order.sort_by_key(|&i| std::cmp::Reverse(ins_of(i).len()));
        let mut paired: HashSet<usize> = HashSet::default();
        let mut readers: HashMap<u32, Vec<usize>> = HashMap::default();
        for &i in &small {
            for &n in ins_of(i) {
                readers.entry(n).or_default().push(i);
            }
        }
        let mut by_size: [Vec<usize>; 6] = Default::default();
        for &i in order.iter().rev() {
            by_size[ins_of(i).len()].push(i);
        }
        let ffs_ok = |a: usize, b: usize| match (own_ff.get(&a), own_ff.get(&b)) {
            (Some(&x), Some(&y)) => compatible(ff_of(x), ff_of(y)),
            _ => true,
        };
        for &u in &order {
            if paired.contains(&u) {
                continue;
            }
            paired.insert(u);
            let iu = ins_of(u);
            let mut score: HashMap<usize, i32> = HashMap::default();
            for n in iu {
                for &v in readers.get(n).map(|v| v.as_slice()).unwrap_or(&[]).iter().take(64) {
                    *score.entry(v).or_default() += 4;
                }
            }
            for &(l, _) in loads.get(&out_of(u)).map(|v| v.as_slice()).unwrap_or(&[]).iter().take(64) {
                *score.entry(l).or_default() += 2;
            }
            for n in iu {
                if let Some(&d) = driver.get(n) {
                    *score.entry(d).or_default() += 2;
                }
            }
            let mut cands: Vec<(i32, usize)> = score
                .into_iter()
                .filter(|(v, _)| *v != u && !paired.contains(v) && small.binary_search(v).is_ok())
                .map(|(v, s)| (s, v))
                .collect();
            cands.sort_by_key(|&(s, v)| (std::cmp::Reverse(s), v));
            let mut pick = None;
            for &(_, v) in &cands {
                if !ffs_ok(u, v) {
                    continue;
                }
                if let Some(p) = fit_halves(iu, ins_of(v)) {
                    pick = Some((v, p));
                    break;
                }
            }
            if pick.is_none() && mode == PackMode::Pairs {
                'size: for k in (0..=(8 - iu.len()).min(5)).rev() {
                    while let Some(&v) = by_size[k].last() {
                        if paired.contains(&v) {
                            by_size[k].pop();
                            continue;
                        }
                        if ffs_ok(u, v) {
                            if let Some(p) = fit_halves(iu, ins_of(v)) {
                                pick = Some((v, p));
                                break 'size;
                            }
                        }
                        break;
                    }
                }
            }
            let mut alm = Alm::empty();
            match pick {
                Some((v, (p0, p1))) => {
                    paired.insert(v);
                    alm.logic = Logic::Five { halves: [Some(Half { lut: HalfLut::Cell(u), pins: p0 }), Some(Half { lut: HalfLut::Cell(v), pins: p1 })] };
                    lut_slots(&mut alm, u, out_of(u), 0);
                    lut_slots(&mut alm, v, out_of(v), 1);
                }
                None => {
                    let (p0, _) = fit_halves(iu, &[]).expect("a LUT of five inputs fits a half");
                    alm.logic = Logic::Five { halves: [Some(Half { lut: HalfLut::Cell(u), pins: p0 }), None] };
                    lut_slots(&mut alm, u, out_of(u), 0);
                }
            }
            alms.push(alm);
        }
    }
    // Every other flop: first in an ALM whose logic it is connected to (its
    // D's driver or one of its Q's loads), then in any ALM with room, then
    // in an ALM of its own.
    let mut alm_of_cell: HashMap<usize, usize> = HashMap::default();
    for (a, alm) in alms.iter().enumerate() {
        match &alm.logic {
            Logic::Six { cell, .. } => {
                alm_of_cell.insert(*cell, a);
            }
            Logic::Five { halves } => {
                for half in halves.iter().flatten() {
                    if let HalfLut::Cell(c) = half.lut {
                        alm_of_cell.insert(c, a);
                    }
                }
            }
            Logic::Arith { halves } => {
                for c in halves.iter().flatten() {
                    alm_of_cell.insert(*c, a);
                }
            }
        }
    }
    let mut open: Vec<usize> = (0..alms.len()).collect();
    // Flops with an enable go into flop-only ALMs kept per enable: spread
    // over logic ALMs, each enable would cost every LAB it touches one of
    // its two enable lines (a register file's 31 enables left the CPU at
    // four ALMs per LAB).
    let mut by_ce: HashMap<u32, Vec<usize>> = HashMap::default();
    let mut rest = Vec::new();
    for (i, c) in nl.cells.iter().enumerate() {
        let Cell::Ff(f) = c else { continue };
        if placed.contains(&i) {
            continue;
        }
        let Sig::Net(d) = f.d else { bail!("flop with a constant D") };
        let Some(ce) = f.ce else {
            rest.push((i, d));
            continue;
        };
        let list = by_ce.entry(ce).or_default();
        let mut done = false;
        for &a in list.iter().rev().take(4) {
            if alms[a].try_flop(nl, i, d).is_some() {
                alm_of_cell.insert(i, a);
                done = true;
                break;
            }
        }
        if !done {
            let mut alm = Alm::empty();
            alm.try_flop(nl, i, d).expect("an empty ALM takes any flop");
            alm_of_cell.insert(i, alms.len());
            list.push(alms.len());
            open.push(alms.len());
            alms.push(alm);
        }
        placed.insert(i);
    }
    for (i, d) in rest {
        let f = ff_of(i);
        let mut near: Vec<usize> = Vec::new();
        if let Some(&dc) = driver.get(&d) {
            near.extend(alm_of_cell.get(&dc));
        }
        for &(l, _) in loads.get(&f.q).map(|v| v.as_slice()).unwrap_or(&[]) {
            near.extend(alm_of_cell.get(&l));
        }
        let mut done = false;
        for &a in &near {
            if alms[a].try_flop(nl, i, d).is_some() {
                alm_of_cell.insert(i, a);
                done = true;
                break;
            }
        }
        if !done {
            // Most recent ALMs first: lone flops fill the ALMs made for them.
            let mut k = open.len();
            while k > 0 {
                k -= 1;
                let a = open[k];
                if alms[a].slots.iter().all(Option::is_some) {
                    open.swap_remove(k);
                    continue;
                }
                if alms[a].try_flop(nl, i, d).is_some() {
                    alm_of_cell.insert(i, a);
                    done = true;
                    break;
                }
                if open.len() - k > 64 {
                    break;
                }
            }
        }
        if !done {
            let mut alm = Alm::empty();
            alm.try_flop(nl, i, d).expect("an empty ALM takes any flop");
            alm_of_cell.insert(i, alms.len());
            open.push(alms.len());
            alms.push(alm);
        }
        placed.insert(i);
    }
    // Nets: drivers and sinks.
    let mut nets: BTreeMap<u32, (End, Vec<End>)> = Default::default();
    let none = End::Port(usize::MAX);
    for (a, alm) in alms.iter().enumerate() {
        let mut sink = |n: u32, e: End| nets.entry(n).or_insert((none, vec![])).1.push(e);
        for (p, n) in alm.pin_nets(nl).iter().enumerate() {
            if let Some(n) = n {
                sink(*n, End::In { alm: a, pin: p as u8 });
            }
        }
        for ctl in alm.ctls(nl) {
            let n = match ctl {
                Ctl::Clock(n) | Ctl::Ce(n) | Ctl::Sclr(n) => n,
                Ctl::Aclr(Some((n, _))) => n,
                Ctl::Aclr(None) => continue,
            };
            sink(n, End::Ctl { alm: a, ctl });
        }
        for s in 0..4 {
            let src = End::Out { alm: a, slot: s as u8 };
            let n = match alm.slots[s] {
                Some(Slot::Ff { ff, .. }) => ff_of(ff).q,
                Some(Slot::Wire { src: Src::Lut }) => match &alm.logic {
                    Logic::Six { cell, .. } => out_of(*cell),
                    Logic::Five { halves } => match halves[s / 2].as_ref().map(|x| x.lut) {
                        Some(HalfLut::Cell(c)) => out_of(c),
                        _ => unreachable!("a wire slot on a half with no LUT cell"),
                    },
                    Logic::Arith { .. } => unreachable!(),
                },
                Some(Slot::Wire { src: Src::Sum }) => {
                    let Logic::Arith { halves } = &alm.logic else { unreachable!() };
                    let Cell::Arith { so: Some(so), .. } = &nl.cells[halves[s / 2].unwrap()] else { unreachable!() };
                    *so
                }
                _ => continue,
            };
            nets.entry(n).or_insert((none, vec![])).0 = src;
        }
    }
    let m20ks: Vec<usize> = nl.cells.iter().enumerate().filter(|(_, c)| matches!(c, Cell::M20k { .. })).map(|(i, _)| i).collect();
    for (m, &ci) in m20ks.iter().enumerate() {
        let Cell::M20k(c) = &nl.cells[ci] else { unreachable!() };
        let mut sink = |s: &Sig, pin: u16, idle: bool| -> Result<()> {
            match s {
                Sig::Net(n) => nets.entry(*n).or_insert((none, vec![])).1.push(End::M20k { m, pin }),
                // An unrouted enable reads as high (Quartus leaves WE, RE and
                // the byte enables of always-on ports unrouted: mm, mq).
                Sig::Const(v) if *v == idle => {}
                Sig::Const(_) => bail!("A5_M20K pin {pin} tied to a constant (netlist::read ties these to LUTs)"),
            }
            Ok(())
        };
        sink(&Sig::Net(c.clk), M20K_CLK, false)?;
        if c.we != Sig::Const(false) {
            sink(&c.we, M20K_WE, true)?;
        }
        sink(&c.re, M20K_RE, true)?;
        for (b, s) in c.be.iter().enumerate() { sink(s, M20K_BE[b], true)?; }
        for (i, s) in c.waddr.iter().enumerate() { sink(s, m20k_waddr(i), false)?; }
        for (i, s) in c.raddr.iter().enumerate() { sink(s, m20k_raddr(i), false)?; }
        for (k, s) in c.wdata.iter().enumerate() { sink(s, m20k_wdata(k), false)?; }
        for (k, n) in c.rdata.iter().enumerate() {
            if let Some(n) = n {
                nets.entry(*n).or_insert((none, vec![])).0 = End::M20k { m, pin: m20k_rdata(k) };
            }
        }
    }
    let plls: Vec<usize> = nl.cells.iter().enumerate().filter(|(_, c)| matches!(c, Cell::Pll { .. })).map(|(i, _)| i).collect();
    for (k, &ci) in plls.iter().enumerate() {
        let Cell::Pll { out, .. } = nl.cells[ci] else { unreachable!() };
        nets.entry(out).or_insert((none, vec![])).0 = End::Pll(k);
    }
    for (pi, p) in nl.ports.iter().enumerate() {
        match (p.input, p.net) {
            (true, Sig::Net(n)) => nets.entry(n).or_insert((End::Port(pi), vec![])).0 = End::Port(pi),
            (false, Sig::Net(n)) => nets.entry(n).or_insert((none, vec![])).1.push(End::Port(pi)),
            (false, Sig::Const(_)) => bail!("output {} is a constant; drive it from logic", p.name),
            _ => {}
        }
    }
    for (n, (d, s)) in &nets {
        if *d == none && !s.is_empty() {
            bail!("net {} has loads but no driver a5pnr can place", nl.nets[*n as usize]);
        }
    }
    nets.retain(|_, (_, s)| !s.is_empty());
    Ok(Packed { alms, chains, m20ks, nets, clocks, plls })
}
