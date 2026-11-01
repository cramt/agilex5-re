//! Packing cells into ALMs (README (w), (y)).
//!
//! An ALM is its logic (one 6-LUT, or two halves of a carry chain) and four
//! register slots. Slot s is register `xcw_la_le_reg{s / 2}/xreg{s % 2}` and
//! drives LAB_RE pin 89 + 4k + s; it is a flop or a wire (WIRE_MODE, the
//! combinational output). A slot's input is the 6-LUT, its half's sum, or
//! (`reg_input` LEIM) the ALM's input pin A for half 0 and B for half 1 [obs,
//! hx and mm's quartus_sta paths]. So a flop with no logic of its own shares
//! an ALM with unrelated logic, as long as that logic leaves the pin free.
//!
//! Rules an ALM must follow (README (y)): the two slots of a half share a
//! clock and an asynchronous clear (`regclk`, `regclr` are per half); every
//! flop with an enable uses the ALM's one enable line (`regclk_ens`); each
//! flop opts in or out of the enable and of the LAB's synchronous clear.

use std::collections::{BTreeMap, HashMap};

use anyhow::{bail, Result};

use crate::netlist::{Aclr, Cell, Ff, Netlist, Sig};

/// A register slot's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Src {
    /// The ALM's 6-LUT (`LUTOUT_6LUT`).
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

#[derive(Clone, Debug)]
pub enum Logic {
    None,
    /// A LUT cell, its inputs on `pins` (positions in A..F, see LUT_IN).
    Lut { cell: usize, pins: Vec<u8> },
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

/// LUT input i (A..F) of an ALM is LAB_RE pin 8k + LUT_IN[i] (README (n)).
pub const LUT_IN: [u8; 6] = [3, 4, 2, 1, 0, 7];
/// An arithmetic half's C and D inputs (README (w)).
pub const ARITH_IN: [[u8; 2]; 2] = [[2, 1], [5, 6]];
/// Position (in A..F) of the pin a half's LEIM slots read: A, B.
pub const LEIM_POS: [u8; 2] = [0, 1];

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
}

impl Alm {
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
        // Only xreg0 has ce_sel and sclr_sel (bcm_types.json; Quartus puts
        // no enabled or cleared flop in xreg1, ct).
        if s % 2 == 1 && (f.ce.is_some() || f.sclr.is_some()) {
            return false;
        }
        let h = s / 2;
        let aclr = |a: Option<Aclr>| a.map(|a| (a.net, a.active_low));
        if let Some(o) = self.ff(nl, 2 * h + 1 - s % 2) {
            if o.clk != f.clk || aclr(o.aclr) != aclr(f.aclr) {
                return false;
            }
        }
        // One enable line per ALM; and what the LAB has only one of (a clock,
        // a synchronous clear, a routed clear: README (y)) can't differ
        // within an ALM either.
        for o in (0..4).filter_map(|t| self.ff(nl, t)) {
            if f.ce.is_some_and(|c| o.ce.is_some_and(|d| d != c))
                || o.clk != f.clk
                || f.sclr.is_some_and(|c| o.sclr.is_some_and(|d| d != c))
                || matches!((aclr(o.aclr), aclr(f.aclr)), (Some(a), Some(b)) if a != b)
            {
                return false;
            }
        }
        true
    }

    /// The net on input position p (A..F), if the logic uses it.
    fn lut_net(&self, nl: &Netlist, p: u8) -> Option<u32> {
        match &self.logic {
            Logic::Lut { cell, pins } => {
                let Cell::Lut { ins, .. } = &nl.cells[*cell] else { return None };
                pins.iter().position(|&q| q == p).map(|i| ins[i])
            }
            _ => None,
        }
    }

    /// The net a half's LEIM slots read, if one already does.
    fn leim_net(&self, nl: &Netlist, h: usize) -> Option<u32> {
        (2 * h..2 * h + 2).find_map(|s| match self.slots[s] {
            Some(Slot::Ff { ff, src: Src::Leim }) => match &nl.cells[ff] {
                Cell::Ff(Ff { d: Sig::Net(d), .. }) => Some(*d),
                _ => None,
            },
            _ => None,
        })
    }

    /// Put flop f, D = net d, into a LEIM slot if the pin rules allow:
    /// the half's pin (A or B) must be free or already carry d, moving a
    /// LUT input off it when the LUT has a spare position.
    fn try_leim(&mut self, nl: &Netlist, ff: usize, d: u32) -> Option<usize> {
        let Cell::Ff(f) = &nl.cells[ff] else { return None };
        for s in 0..4 {
            let h = s / 2;
            if !self.fits(nl, s, f) {
                continue;
            }
            if self.leim_net(nl, h).is_some_and(|n| n != d) {
                continue;
            }
            let p = LEIM_POS[h];
            // The LUT's positions after making room, if it can.
            let new_pins = match &self.logic {
                Logic::None | Logic::Arith { .. } => None,
                Logic::Lut { cell, pins } => {
                    let Cell::Lut { ins, .. } = &nl.cells[*cell] else { unreachable!() };
                    let mut pins = pins.clone();
                    match pins.iter().position(|&q| q == p) {
                        None => {}
                        Some(i) if ins[i] == d => {}
                        Some(i) => {
                            // Swap with the input already carrying d, or move to a free position.
                            if let Some(j) = ins.iter().position(|&n| n == d) {
                                pins.swap(i, j);
                            } else if let Some(free) = (0..6u8).rev().find(|q| !pins.contains(q) && !LEIM_POS.contains(q)) {
                                pins[i] = free;
                            } else {
                                continue;
                            }
                        }
                    }
                    // The other half's LEIM pin must still carry its net.
                    let other = LEIM_POS[1 - h];
                    if let Some(n) = self.leim_net(nl, 1 - h) {
                        if pins.iter().position(|&q| q == other).is_some_and(|i| ins[i] != n) {
                            continue;
                        }
                    }
                    Some(pins)
                }
            };
            if let (Some(np), Logic::Lut { pins, .. }) = (new_pins, &mut self.logic) {
                *pins = np;
            }
            self.slots[s] = Some(Slot::Ff { ff, src: Src::Leim });
            return Some(s);
        }
        None
    }
}

/// LUT inputs on positions, the fastest last (F, E, … in the delay model,
/// README (u)) and A, B left for LEIM flops while there is room.
fn lut_positions(n: usize) -> Vec<u8> {
    const ORDER: [u8; 6] = [5, 4, 3, 2, 1, 0];
    ORDER[..n].to_vec()
}

pub fn pack(nl: &Netlist) -> Result<Packed> {
    let mut driver: HashMap<u32, usize> = HashMap::new();
    let mut loads: HashMap<u32, Vec<(usize, &'static str)>> = HashMap::new();
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
    let mut placed: std::collections::HashSet<usize> = Default::default();
    let mut alms: Vec<Alm> = Vec::new();
    // The first flop on a net, when it can go with that net's driver.
    let mut ffs_on: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Ff(Ff { d: Sig::Net(d), .. }) = c {
            ffs_on.entry(*d).or_default().push(i);
        }
    }
    // Carry chains, from their heads.
    let mut next: HashMap<usize, usize> = HashMap::new();
    let mut has_prev = std::collections::HashSet::new();
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Arith { co: Some(co), .. } = c {
            for &(l, port) in loads.get(co).map(|v| v.as_slice()).unwrap_or(&[]) {
                if port != "ci" {
                    bail!("a carry out feeds something other than a carry in");
                }
                next.insert(i, l);
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
                let ff = ff.filter(|&f| matches!(&nl.cells[f], Cell::Ff(x) if alm.fits(nl, 2 * h, x)));
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
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Lut { ins, out, .. } = c {
            let mut alm = Alm { logic: Logic::Lut { cell: i, pins: lut_positions(ins.len()) }, slots: [None; 4] };
            let ff = ffs_on.get(out).and_then(|v| v.iter().copied().find(|f| !placed.contains(f)));
            if let Some(f) = ff {
                alm.slots[0] = Some(Slot::Ff { ff: f, src: Src::Lut });
                placed.insert(f);
            }
            if comb_used(*out, ff) {
                alm.slots[1] = Some(Slot::Wire { src: Src::Lut });
            }
            alms.push(alm);
        }
    }
    // Every other flop reads its D on a LEIM slot: first in an ALM whose
    // logic it is connected to (its D's driver or one of its Q's loads),
    // then in any ALM with room, then in an ALM of its own.
    let mut alm_of_cell: HashMap<usize, usize> = HashMap::new();
    for (a, alm) in alms.iter().enumerate() {
        match &alm.logic {
            Logic::Lut { cell, .. } => {
                alm_of_cell.insert(*cell, a);
            }
            Logic::Arith { halves } => {
                for c in halves.iter().flatten() {
                    alm_of_cell.insert(*c, a);
                }
            }
            Logic::None => {}
        }
    }
    let mut open: Vec<usize> = (0..alms.len()).collect();
    for (i, c) in nl.cells.iter().enumerate() {
        let Cell::Ff(f) = c else { continue };
        if placed.contains(&i) {
            continue;
        }
        let Sig::Net(d) = f.d else { bail!("flop with a constant D") };
        let mut near: Vec<usize> = Vec::new();
        if let Some(&dc) = driver.get(&d) {
            near.extend(alm_of_cell.get(&dc));
        }
        for &(l, _) in loads.get(&f.q).map(|v| v.as_slice()).unwrap_or(&[]) {
            near.extend(alm_of_cell.get(&l));
        }
        let mut done = false;
        for &a in &near {
            if alms[a].try_leim(nl, i, d).is_some() {
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
                if alms[a].try_leim(nl, i, d).is_some() {
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
            let mut alm = Alm { logic: Logic::None, slots: [None; 4] };
            alm.try_leim(nl, i, d).expect("an empty ALM takes any flop");
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
        match &alm.logic {
            Logic::Lut { cell, pins } => {
                let Cell::Lut { ins, .. } = &nl.cells[*cell] else { unreachable!() };
                for (k, n) in ins.iter().enumerate() {
                    sink(*n, End::In { alm: a, pin: LUT_IN[pins[k] as usize] });
                }
            }
            Logic::Arith { halves } => {
                for (h, half) in halves.iter().enumerate() {
                    let Some(c) = half else { continue };
                    let Cell::Arith { c, d, .. } = &nl.cells[*c] else { unreachable!() };
                    for (k, n) in [c, d].into_iter().enumerate() {
                        if let Some(n) = n {
                            sink(*n, End::In { alm: a, pin: ARITH_IN[h][k] });
                        }
                    }
                }
            }
            Logic::None => {}
        }
        for h in 0..2 {
            let mut leim = None;
            for s in 2 * h..2 * h + 2 {
                if let Some(Slot::Ff { ff, src: Src::Leim }) = alm.slots[s] {
                    let Cell::Ff(Ff { d: Sig::Net(d), .. }) = nl.cells[ff] else { unreachable!() };
                    leim = Some(d);
                }
            }
            // A LUT input already on the pin brings the net there too.
            if let Some(d) = leim {
                if alm.lut_net(nl, LEIM_POS[h]) != Some(d) {
                    sink(d, End::In { alm: a, pin: LUT_IN[LEIM_POS[h] as usize] });
                }
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
                Some(Slot::Ff { ff, .. }) => {
                    let Cell::Ff(f) = &nl.cells[ff] else { unreachable!() };
                    f.q
                }
                Some(Slot::Wire { src: Src::Lut }) => {
                    let Logic::Lut { cell, .. } = &alm.logic else { unreachable!() };
                    let Cell::Lut { out, .. } = &nl.cells[*cell] else { unreachable!() };
                    *out
                }
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
    Ok(Packed { alms, chains, m20ks, nets, clocks })
}
