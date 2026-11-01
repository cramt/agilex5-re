//! Packing cells into ALMs and placing ALMs in LABs (README (w)).
//!
//! An ALM holds either one LUT (up to 6 inputs, ALM half 0) with its flop and
//! a combinational output, or two halves of a carry chain, each with its flop
//! and/or combinational sum. A carry chain runs through the half-ALM slots of
//! a LAB in order (ALM 0 half 0, half 1, ALM 1 …, 20 per LAB) and on into the
//! LAB one row down; chains start at ALM 0 of a LAB.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::netlist::{Cell, Netlist, Sig};

#[derive(Clone, Debug)]
pub struct Half {
    pub cell: usize,
    pub ff: Option<usize>,
    pub comb: bool,
}

#[derive(Clone, Debug)]
pub enum Alm {
    /// `lut` None: a pass-through LUT (output = A) in front of a lone flop.
    Lut { lut: Option<usize>, ff: Option<usize>, comb: bool },
    Arith { halves: [Option<Half>; 2] },
}

/// Where a net comes from or goes to, before placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// ALM a's output: half h, xreg0 (the flop) or xreg1 (combinational).
    Out { alm: usize, h: u8, ff: bool },
    /// ALM a's input pin, as the offset within the ALM's 8 LAB_RE pins.
    In { alm: usize, pin: u8 },
    /// The LAB clock of ALM a's LAB.
    Clock { alm: usize },
    /// A top-level port.
    Port(usize),
    /// Pin of M20K m (`MEDIUM_EAB_RE` index), input or output by context.
    M20k { m: usize, pin: u16 },
}

/// The M20K's pins in its 512 x 32 simple dual-port use (README (w)), from
/// mm's quartus_sta paths: clock, write and read address, write and read data.
pub const M20K_CLK: u16 = 77;
pub fn m20k_waddr(i: usize) -> u16 { 25 + i as u16 }
pub fn m20k_raddr(i: usize) -> u16 { 52 - i as u16 }
pub fn m20k_wdata(k: usize) -> u16 { if k < 20 { 57 + k as u16 } else { 40 - k as u16 } }
pub fn m20k_rdata(k: usize) -> u16 { 83 + k as u16 }

pub struct Packed {
    pub alms: Vec<Alm>,
    /// Chains as ALM lists, in chain order.
    pub chains: Vec<Vec<usize>>,
    /// M20K cells, by netlist index.
    pub m20ks: Vec<usize>,
    /// Per net: driver and sinks.
    pub nets: std::collections::BTreeMap<u32, (End, Vec<End>)>,
    pub clock: Option<u32>,
}

/// LUT input i (A..F) of an ALM is LAB_RE pin 8k + IN[i] (README (n)).
pub const LUT_IN: [u8; 6] = [3, 4, 2, 1, 0, 7];
/// An arithmetic half's C and D inputs (README (w)).
pub const ARITH_IN: [[u8; 2]; 2] = [[2, 1], [5, 6]];

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
            Cell::Ff { d, clk, q } => {
                driver.insert(*q, i);
                add(*d, "d");
                add(Sig::Net(*clk), "clk");
            }
            Cell::M20k { clk, we, waddr, wdata, raddr, rdata } => {
                if *we != Sig::Const(true) {
                    bail!("A5_M20K: only an always-written RAM (WE = 1) is modelled");
                }
                for s in waddr.iter().chain(wdata).chain(raddr) {
                    add(*s, "m20k");
                }
                add(Sig::Net(*clk), "clk");
                for n in rdata.iter().flatten() {
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
    let clocks: Vec<u32> = nl.cells.iter().filter_map(|c| match c { Cell::Ff { clk, .. } | Cell::M20k { clk, .. } => Some(*clk), _ => None }).collect();
    let clock = clocks.first().copied();
    if clocks.iter().any(|c| Some(*c) != clock) {
        bail!("more than one clock: a5pnr handles one clock domain");
    }
    // A flop goes with the LUT or sum that drives it, when that output feeds
    // only the flop; otherwise the LUT's combinational output is used too.
    let mut ff_of: HashMap<usize, usize> = HashMap::new();
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Ff { d: Sig::Net(d), .. } = c {
            if let Some(&src) = driver.get(d) {
                let is_so = matches!(nl.cells[src], Cell::Arith { so: Some(s), .. } if s == *d);
                if matches!(nl.cells[src], Cell::Lut { .. }) || is_so {
                    if !ff_of.contains_key(&src) {
                        ff_of.insert(src, i);
                        continue;
                    }
                }
            }
        }
    }
    let placed_ff: std::collections::HashSet<usize> = ff_of.values().copied().collect();
    let comb_used = |net: u32, ff: Option<usize>| -> bool {
        loads.get(&net).is_some_and(|l| l.iter().any(|&(c, port)| !(Some(c) == ff && port == "d")))
    };
    let mut alms = Vec::new();
    let mut cell_alm: HashMap<usize, (usize, u8)> = HashMap::new();
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
        let mut chain = Vec::new();
        for pair in cells.chunks(2) {
            let mut halves: [Option<Half>; 2] = [None, None];
            for (h, &ci) in pair.iter().enumerate() {
                let ff = ff_of.get(&ci).copied();
                let comb = match &nl.cells[ci] {
                    Cell::Arith { so: Some(so), .. } => comb_used(*so, ff),
                    _ => false,
                };
                halves[h] = Some(Half { cell: ci, ff, comb });
                cell_alm.insert(ci, (alms.len(), h as u8));
                if let Some(f) = ff {
                    cell_alm.insert(f, (alms.len(), h as u8));
                }
            }
            chain.push(alms.len());
            alms.push(Alm::Arith { halves });
        }
        chains.push(chain);
    }
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Lut { out, .. } = c {
            let ff = ff_of.get(&i).copied();
            let comb = comb_used(*out, ff);
            cell_alm.insert(i, (alms.len(), 0));
            if let Some(f) = ff {
                cell_alm.insert(f, (alms.len(), 0));
            }
            alms.push(Alm::Lut { lut: Some(i), ff, comb });
        }
    }
    // A flop with no LUT of its own gets a pass-through LUT (lut_mask = A).
    for (i, c) in nl.cells.iter().enumerate() {
        if let Cell::Ff { d: Sig::Net(_), .. } = c {
            if !placed_ff.contains(&i) {
                cell_alm.insert(i, (alms.len(), 0));
                alms.push(Alm::Lut { lut: None, ff: Some(i), comb: false });
            }
        }
    }
    // Nets: drivers and sinks.
    let mut nets: std::collections::BTreeMap<u32, (End, Vec<End>)> = Default::default();
    for (a, alm) in alms.iter().enumerate() {
        match alm {
            Alm::Lut { lut, ff, comb } => {
                if let Some(lut) = lut {
                    let Cell::Lut { ins, out, .. } = &nl.cells[*lut] else { unreachable!() };
                    if *comb {
                        nets.entry(*out).or_insert((End::Out { alm: a, h: 0, ff: false }, vec![])).0 = End::Out { alm: a, h: 0, ff: false };
                    }
                    for (k, n) in ins.iter().enumerate() {
                        nets.entry(*n).or_insert((End::Port(usize::MAX), vec![])).1.push(End::In { alm: a, pin: LUT_IN[k] });
                    }
                } else {
                    let Cell::Ff { d: Sig::Net(d), .. } = nl.cells[ff.unwrap()] else { unreachable!() };
                    nets.entry(d).or_insert((End::Port(usize::MAX), vec![])).1.push(End::In { alm: a, pin: LUT_IN[0] });
                }
                if let Some(f) = ff {
                    let Cell::Ff { q, .. } = nl.cells[*f] else { unreachable!() };
                    nets.entry(q).or_insert((End::Port(usize::MAX), vec![])).0 = End::Out { alm: a, h: 0, ff: true };
                }
            }
            Alm::Arith { halves } => {
                for (h, half) in halves.iter().enumerate() {
                    let Some(half) = half else { continue };
                    let Cell::Arith { c, d, so, .. } = &nl.cells[half.cell] else { unreachable!() };
                    for (k, n) in [c, d].into_iter().enumerate() {
                        if let Some(n) = n {
                            nets.entry(*n).or_insert((End::Port(usize::MAX), vec![])).1.push(End::In { alm: a, pin: ARITH_IN[h][k] });
                        }
                    }
                    if half.comb {
                        nets.entry(so.unwrap()).or_insert((End::Port(usize::MAX), vec![])).0 = End::Out { alm: a, h: h as u8, ff: false };
                    }
                    if let Some(f) = half.ff {
                        let Cell::Ff { q, .. } = nl.cells[f] else { unreachable!() };
                        nets.entry(q).or_insert((End::Port(usize::MAX), vec![])).0 = End::Out { alm: a, h: h as u8, ff: true };
                    }
                }
            }
        }
    }
    let m20ks: Vec<usize> = nl.cells.iter().enumerate().filter(|(_, c)| matches!(c, Cell::M20k { .. })).map(|(i, _)| i).collect();
    for (m, &ci) in m20ks.iter().enumerate() {
        let Cell::M20k { waddr, wdata, raddr, rdata, .. } = &nl.cells[ci] else { unreachable!() };
        let mut sink = |s: &Sig, pin: u16| -> Result<()> {
            match s {
                Sig::Net(n) => nets.entry(*n).or_insert((End::Port(usize::MAX), vec![])).1.push(End::M20k { m, pin }),
                // A constant address or data bit would need a tie-off: not modelled.
                Sig::Const(_) => bail!("A5_M20K input tied to a constant"),
            }
            Ok(())
        };
        for (i, s) in waddr.iter().enumerate() { sink(s, m20k_waddr(i))?; }
        for (i, s) in raddr.iter().enumerate() { sink(s, m20k_raddr(i))?; }
        for (k, s) in wdata.iter().enumerate() { sink(s, m20k_wdata(k))?; }
        for (k, n) in rdata.iter().enumerate() {
            if let Some(n) = n {
                nets.entry(*n).or_insert((End::Port(usize::MAX), vec![])).0 = End::M20k { m, pin: m20k_rdata(k) };
            }
        }
    }
    for (pi, p) in nl.ports.iter().enumerate() {
        match (p.input, p.net) {
            (true, Sig::Net(n)) => nets.entry(n).or_insert((End::Port(pi), vec![])).0 = End::Port(pi),
            (false, Sig::Net(n)) => nets.entry(n).or_insert((End::Port(usize::MAX), vec![])).1.push(End::Port(pi)),
            (false, Sig::Const(_)) => bail!("output {} is a constant; drive it from logic", p.name),
            _ => {}
        }
    }
    if let Some(clk) = clock {
        let e = nets.entry(clk).or_insert((End::Port(usize::MAX), vec![]));
        let mut labs_seen = Vec::new();
        for (a, alm) in alms.iter().enumerate() {
            let has_ff = match alm {
                Alm::Lut { ff, .. } => ff.is_some(),
                Alm::Arith { halves } => halves.iter().flatten().any(|h| h.ff.is_some()),
            };
            if has_ff {
                labs_seen.push(a);
            }
        }
        e.1.extend(labs_seen.into_iter().map(|a| End::Clock { alm: a }));
        e.1.extend((0..m20ks.len()).map(|m| End::M20k { m, pin: M20K_CLK }));
    }
    for (n, (d, s)) in &nets {
        if *d == End::Port(usize::MAX) && !s.is_empty() {
            bail!("net {} has loads but no driver a5pnr can place", nl.nets[*n as usize]);
        }
    }
    nets.retain(|_, (_, s)| !s.is_empty());
    let _ = cell_alm;
    Ok(Packed { alms, chains, m20ks, nets, clock })
}

/// A LAB site.
#[derive(Clone, Copy, Debug)]
pub struct Lab {
    pub x: u32,
    pub y: u32,
}

pub fn labs(db: &str) -> Result<Vec<Lab>> {
    let text = std::fs::read_to_string(format!("{db}/floorplan.tsv")).context("floorplan.tsv")?;
    let mut out = Vec::new();
    for l in text.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f[1] == "cw_la_lab" && !f[0].contains("rsp") {
            out.push(Lab { x: f[3].parse()?, y: f[4].parse()? });
        }
    }
    out.sort_by_key(|l| (l.x, l.y));
    Ok(out)
}

/// A tiny deterministic generator (xorshift), so runs repeat.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

pub struct Placement {
    /// Per ALM: (LAB index, k).
    pub at: Vec<(usize, u8)>,
    /// Per M20K: its block's X, Y.
    pub m20k_at: Vec<(u32, u32)>,
}

pub fn m20k_sites(db: &str) -> Result<Vec<(u32, u32)>> {
    let text = std::fs::read_to_string(format!("{db}/floorplan.tsv")).context("floorplan.tsv")?;
    let mut out = Vec::new();
    for l in text.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f[1] == "cw_mm_m20k" && !f[0].contains("rsp") {
            out.push((f[3].parse()?, f[4].parse()?));
        }
    }
    Ok(out)
}

/// Simulated annealing on half-perimeter wire length, in LAB coordinates.
/// Port positions are their pin nodes' X/Y.
pub fn place(p: &Packed, labs: &[Lab], m20k_sites: &[(u32, u32)], port_xy: &HashMap<usize, (u32, u32)>, seed: u64) -> Result<Placement> {
    let mut rng = Rng::new(seed);
    let lab_at: HashMap<(u32, u32), usize> = labs.iter().enumerate().map(|(i, l)| ((l.x, l.y), i)).collect();
    let mut occ: HashMap<(usize, u8), usize> = HashMap::new();
    let mut at = vec![(usize::MAX, 0u8); p.alms.len()];
    // Start near the ports' centre.
    let (cx, cy) = if port_xy.is_empty() {
        (60, 28)
    } else {
        let n = port_xy.len() as u32;
        (port_xy.values().map(|v| v.0).sum::<u32>() / n, port_xy.values().map(|v| v.1).sum::<u32>() / n)
    };
    // M20Ks go to the sites nearest the ports' centre and stay there.
    let mut sites: Vec<(u32, u32)> = m20k_sites.to_vec();
    sites.sort_by_key(|&(x, y)| (x as i64 - cx as i64).abs() + (y as i64 - cy as i64).abs());
    if p.m20ks.len() > sites.len() {
        bail!("{} M20Ks, {} sites", p.m20ks.len(), sites.len());
    }
    let m20k_at: Vec<(u32, u32)> = sites[..p.m20ks.len()].to_vec();
    let mut by_dist: Vec<usize> = (0..labs.len()).collect();
    by_dist.sort_by_key(|&i| (labs[i].x as i64 - cx as i64).abs() + (labs[i].y as i64 - cy as i64).abs());
    let in_chain: std::collections::HashSet<usize> = p.chains.iter().flatten().copied().collect();
    // Chains first: each into LABs (x, y), (x, y-1), … at ALM 0 on.
    let chain_fits = |occ: &HashMap<(usize, u8), usize>, x: u32, y: u32, n: usize| -> Option<Vec<(usize, u8)>> {
        let mut slots = Vec::new();
        for j in 0..n {
            let l = *lab_at.get(&(x, y.checked_sub((j / 10) as u32)?))?;
            let s = (l, (j % 10) as u8);
            if occ.contains_key(&s) {
                return None;
            }
            slots.push(s);
        }
        Some(slots)
    };
    for (ci, c) in p.chains.iter().enumerate() {
        let mut done = false;
        for &l in &by_dist {
            if let Some(slots) = chain_fits(&occ, labs[l].x, labs[l].y, c.len()) {
                for (j, s) in slots.into_iter().enumerate() {
                    occ.insert(s, c[j]);
                    at[c[j]] = s;
                }
                done = true;
                break;
            }
        }
        if !done {
            bail!("chain {ci} ({} ALMs) doesn't fit", c.len());
        }
    }
    let mut free = by_dist.iter().flat_map(|&l| (0..10u8).map(move |k| (l, k)));
    for a in 0..p.alms.len() {
        if in_chain.contains(&a) {
            continue;
        }
        let s = loop {
            let s = free.next().context("design doesn't fit")?;
            if !occ.contains_key(&s) {
                break s;
            }
        };
        occ.insert(s, a);
        at[a] = s;
    }
    // Annealing over single ALMs (chains stay put).
    let nets: Vec<&(End, Vec<End>)> = p.nets.iter().filter(|(n, _)| Some(**n) != p.clock).map(|(_, v)| v).collect();
    let mut alm_nets: Vec<Vec<usize>> = vec![Vec::new(); p.alms.len()];
    for (i, (d, s)) in nets.iter().enumerate() {
        for e in std::iter::once(d).chain(s.iter()) {
            match e {
                End::Out { alm, .. } | End::In { alm, .. } | End::Clock { alm } => alm_nets[*alm].push(i),
                End::Port(_) | End::M20k { .. } => {}
            }
        }
    }
    for v in &mut alm_nets {
        v.sort();
        v.dedup();
    }
    let xy = |at: &[(usize, u8)], e: &End| -> Option<(i64, i64)> {
        match e {
            End::Out { alm, .. } | End::In { alm, .. } | End::Clock { alm } => {
                let l = labs[at[*alm].0];
                Some((l.x as i64, l.y as i64))
            }
            End::Port(p) => port_xy.get(p).map(|&(x, y)| (x as i64, y as i64)),
            End::M20k { m, .. } => Some((m20k_at[*m].0 as i64, m20k_at[*m].1 as i64)),
        }
    };
    let hpwl = |at: &[(usize, u8)], i: usize| -> i64 {
        let (d, s) = nets[i];
        let pts: Vec<(i64, i64)> = std::iter::once(d).chain(s.iter()).filter_map(|e| xy(at, e)).collect();
        if pts.is_empty() {
            return 0;
        }
        let (x0, x1) = (pts.iter().map(|p| p.0).min().unwrap(), pts.iter().map(|p| p.0).max().unwrap());
        let (y0, y1) = (pts.iter().map(|p| p.1).min().unwrap(), pts.iter().map(|p| p.1).max().unwrap());
        (x1 - x0) + (y1 - y0)
    };
    let movable: Vec<usize> = (0..p.alms.len()).filter(|a| !in_chain.contains(a)).collect();
    if movable.is_empty() || nets.is_empty() {
        return Ok(Placement { at, m20k_at });
    }
    let mut cost: i64 = (0..nets.len()).map(|i| hpwl(&at, i)).sum();
    let mut temp = 10.0f64;
    let moves = 2000 * movable.len().max(10);
    let window = 12i64;
    for step in 0..moves {
        let a = movable[rng.below(movable.len())];
        let (l0, _) = at[a];
        let (x0, y0) = (labs[l0].x as i64, labs[l0].y as i64);
        let tx = x0 + rng.below(2 * window as usize + 1) as i64 - window;
        let ty = y0 + rng.below(2 * window as usize + 1) as i64 - window;
        let Some(&tl) = lab_at.get(&(tx.max(0) as u32, ty.max(0) as u32)) else { continue };
        let tk = rng.below(10) as u8;
        let other = occ.get(&(tl, tk)).copied();
        if other.is_some_and(|o| in_chain.contains(&o)) || other == Some(a) {
            continue;
        }
        let mut touched: Vec<usize> = alm_nets[a].clone();
        if let Some(o) = other {
            touched.extend(&alm_nets[o]);
            touched.sort();
            touched.dedup();
        }
        let before: i64 = touched.iter().map(|&i| hpwl(&at, i)).sum();
        let old = at[a];
        at[a] = (tl, tk);
        if let Some(o) = other {
            at[o] = old;
        }
        let after: i64 = touched.iter().map(|&i| hpwl(&at, i)).sum();
        let delta = after - before;
        if delta <= 0 || rng.unit() < (-(delta as f64) / temp).exp() {
            cost += delta;
            occ.remove(&old);
            occ.insert((tl, tk), a);
            if let Some(o) = other {
                occ.insert(old, o);
            }
        } else {
            at[a] = old;
            if let Some(o) = other {
                at[o] = (tl, tk);
            }
        }
        if step % 1000 == 999 {
            temp *= 0.97;
        }
    }
    eprintln!("place: {} ALMs ({} in {} chains), HPWL {cost}", p.alms.len(), in_chain.len(), p.chains.len());
    Ok(Placement { at, m20k_at })
}
