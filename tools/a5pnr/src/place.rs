//! Placing ALMs in LABs (README (w), (y)): simulated annealing on wire
//! length plus, when timing is known, criticality-weighted connection delay
//! (README (z)), with a VPR-style schedule (temperature from the acceptance
//! rate, a shrinking move range). A carry chain runs through the half-ALM
//! slots of a LAB in order (ALM 0 half 0, half 1, ALM 1 …, 20 per LAB) and
//! on into the LAB one row down, so a chain starts at ALM 0 of a LAB and
//! moves as a whole. A LAB has two clock lines, two enable lines, one
//! synchronous clear and two asynchronous clears (README (y)); no move may
//! ask a LAB for more.

use crate::det::HashMap;

use anyhow::{bail, Context, Result};

use crate::netlist::Netlist;
use crate::pack::{Ctl, End, Packed};

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

/// A tiny deterministic generator (xorshift), so runs repeat.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9e3779b97f4a7c15) | 1)
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

/// How many of each control line a LAB's ALMs ask for, and whether that
/// fits: two clocks, two enables, one synchronous clear, two asynchronous
/// clears (an ALM without one still takes a line, left inactive).
#[derive(Default, Clone)]
struct LabUse(HashMap<Ctl, u32>);

impl LabUse {
    fn add(&mut self, ctls: &[Ctl], d: i32) {
        for c in ctls {
            let e = self.0.entry(*c).or_default();
            *e = (*e as i32 + d) as u32;
            if *e == 0 {
                self.0.remove(c);
            }
        }
    }
    /// One clock (Quartus used LAB clock 0 only, also with two domains:
    /// ct), two enables, one synchronous clear, one routed asynchronous
    /// clear (CLR1, LAB_RE 86) and the other line inactive (README (y)).
    fn ok(&self) -> bool {
        let (mut clk, mut ce, mut sclr, mut aclr) = (0, 0, 0, 0);
        for c in self.0.keys() {
            match c {
                Ctl::Clock(_) => clk += 1,
                Ctl::Ce(_) => ce += 1,
                Ctl::Sclr(_) => sclr += 1,
                Ctl::Aclr(Some(_)) => aclr += 1,
                Ctl::Aclr(None) => {}
            }
        }
        clk <= 1 && ce <= 2 && sclr <= 1 && aclr <= 1
    }
}

/// VPR's wire-length correction for a net's bounding box by pin count.
fn crossing(pins: usize) -> f64 {
    const Q: [f64; 50] = [
        1.0, 1.0, 1.0, 1.0828, 1.1536, 1.2206, 1.2823, 1.3385, 1.3991, 1.4493, 1.4974, 1.5455, 1.5937, 1.6418, 1.6899, 1.7304,
        1.7709, 1.8114, 1.8519, 1.8924, 1.9288, 1.9652, 2.0015, 2.0379, 2.0743, 2.1061, 2.1379, 2.1698, 2.2016, 2.2334, 2.2646,
        2.2958, 2.3271, 2.3583, 2.3895, 2.4187, 2.4479, 2.4772, 2.5064, 2.5356, 2.5610, 2.5864, 2.6117, 2.6371, 2.6625, 2.6887,
        2.7148, 2.7410, 2.7671, 2.7933,
    ];
    if pins < 50 {
        Q[pins]
    } else {
        2.7933 + 0.02616 * (pins - 50) as f64
    }
}

/// A placement cost term for timing: for each (driver ALM or fixed point,
/// sink ALM) connection, its weight (criticality^exponent) and the delay
/// estimate as a function of the distance.
pub struct TimingCost<'a> {
    /// Per connection: driver end, sink end, weight.
    pub conns: Vec<(End, End, f64)>,
    pub delay: &'a dyn Fn(i64, i64) -> f64,
    pub lambda: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn place(
    p: &Packed,
    nl: &Netlist,
    labs: &[Lab],
    m20k_sites: &[(u32, u32)],
    port_xy: &HashMap<usize, (u32, u32)>,
    seed: u64,
    tc: Option<&TimingCost>,
    effort: f64,
) -> Result<Placement> {
    let mut rng = Rng::new(seed);
    let lab_at: HashMap<(u32, u32), usize> = labs.iter().enumerate().map(|(i, l)| ((l.x, l.y), i)).collect();
    let n = p.alms.len();
    let ctls: Vec<Vec<Ctl>> = p.alms.iter().map(|a| a.ctls(nl)).collect();
    let mut occ: HashMap<(usize, u8), usize> = HashMap::default();
    let mut uses: Vec<LabUse> = vec![LabUse::default(); labs.len()];
    let mut at = vec![(usize::MAX, 0u8); n];
    // Start near the ports' centre.
    let (cx, cy) = if port_xy.is_empty() {
        (60, 28)
    } else {
        let k = port_xy.len() as u32;
        (port_xy.values().map(|v| v.0).sum::<u32>() / k, port_xy.values().map(|v| v.1).sum::<u32>() / k)
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
    let mut chain_of = vec![usize::MAX; n];
    for (ci, c) in p.chains.iter().enumerate() {
        for &a in c {
            chain_of[a] = ci;
        }
    }
    // A chain's slots from LAB (x, y) ALM k0, if all free and legal. Chains
    // start at ALM 0: every Quartus chain does (cql's six, README (z4)), and
    // Quartus' fitter rejects one that starts at ALM 5 and runs on.
    let chain_slots = |occ: &HashMap<(usize, u8), usize>, uses: &[LabUse], c: &[usize], x: u32, y: u32, k0: usize| -> Option<Vec<(usize, u8)>> {
        let mut slots = Vec::new();
        let mut trial: HashMap<usize, LabUse> = HashMap::default();
        for (j, &a) in c.iter().enumerate() {
            let l = *lab_at.get(&(x, y.checked_sub(((k0 + j) / 10) as u32)?))?;
            let s = (l, ((k0 + j) % 10) as u8);
            if occ.get(&s).is_some_and(|&o| !c.contains(&o)) {
                return None;
            }
            trial.entry(l).or_insert_with(|| uses[l].clone()).add(&ctls[a], 1);
            slots.push(s);
        }
        trial.values().all(LabUse::ok).then_some(slots)
    };
    for (ci, c) in p.chains.iter().enumerate() {
        let slots = by_dist
            .iter()
            .map(|&l| (l, 0))
            .find_map(|(l, k0)| chain_slots(&occ, &uses, c, labs[l].x, labs[l].y, k0))
            .with_context(|| format!("chain {ci} ({} ALMs) doesn't fit; its control lines: {:?}", c.len(), c.iter().map(|&a| &ctls[a]).collect::<Vec<_>>()))?;
        for (j, s) in slots.into_iter().enumerate() {
            occ.insert(s, c[j]);
            at[c[j]] = s;
            uses[s.0].add(&ctls[c[j]], 1);
        }
    }
    {
        let mut li = 0;
        for a in 0..n {
            if chain_of[a] != usize::MAX {
                continue;
            }
            'find: loop {
                let l = *by_dist.get(li).context("design doesn't fit")?;
                for k in 0..10u8 {
                    if !occ.contains_key(&(l, k)) {
                        let mut u = uses[l].clone();
                        u.add(&ctls[a], 1);
                        if u.ok() {
                            occ.insert((l, k), a);
                            at[a] = (l, k);
                            uses[l] = u;
                            break 'find;
                        }
                    }
                }
                li += 1;
            }
        }
    }
    // Nets for the cost: every net but the clocks (they ride the clock network).
    let nets: Vec<&(End, Vec<End>)> = p.nets.iter().filter(|(k, _)| !p.clocks.contains(k)).map(|(_, v)| v).collect();
    let alm_of = |e: &End| -> Option<usize> {
        match e {
            End::Out { alm, .. } | End::In { alm, .. } | End::Ctl { alm, .. } => Some(*alm),
            End::Port(_) | End::M20k { .. } | End::Pll(_) => None,
        }
    };
    let mut alm_nets: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, (d, s)) in nets.iter().enumerate() {
        for e in std::iter::once(d).chain(s.iter()) {
            if let Some(a) = alm_of(e) {
                alm_nets[a].push(i);
            }
        }
    }
    for v in &mut alm_nets {
        v.sort();
        v.dedup();
    }
    let weight: Vec<f64> = nets.iter().map(|(_, s)| crossing(s.len() + 1)).collect();
    let fixed_xy = |e: &End| -> Option<(i64, i64)> {
        match e {
            End::Port(p) => port_xy.get(p).map(|&(x, y)| (x as i64, y as i64)),
            End::M20k { m, .. } => Some((m20k_at[*m].0 as i64, m20k_at[*m].1 as i64)),
            _ => None,
        }
    };
    let xy = |at: &[(usize, u8)], e: &End| -> Option<(i64, i64)> {
        match alm_of(e) {
            Some(a) => {
                let l = labs[at[a].0];
                Some((l.x as i64, l.y as i64))
            }
            None => fixed_xy(e),
        }
    };
    let net_cost = |at: &[(usize, u8)], i: usize| -> f64 {
        let (d, s) = nets[i];
        let (mut x0, mut x1, mut y0, mut y1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
        for e in std::iter::once(d).chain(s.iter()) {
            if let Some((x, y)) = xy(at, e) {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
        if x0 > x1 {
            return 0.0;
        }
        weight[i] * ((x1 - x0) + (y1 - y0)) as f64
    };
    // Timing connections touching each ALM.
    let mut alm_conns: Vec<Vec<usize>> = vec![Vec::new(); n];
    if let Some(tc) = tc {
        for (i, (d, s, _)) in tc.conns.iter().enumerate() {
            for e in [d, s] {
                if let Some(a) = alm_of(e) {
                    alm_conns[a].push(i);
                }
            }
        }
        for v in &mut alm_conns {
            v.sort();
            v.dedup();
        }
    }
    let conn_cost = |at: &[(usize, u8)], i: usize| -> f64 {
        let tc = tc.unwrap();
        let (d, s, w) = &tc.conns[i];
        match (xy(at, d), xy(at, s)) {
            (Some(a), Some(b)) => w * (tc.delay)((a.0 - b.0).abs(), (a.1 - b.1).abs()),
            _ => 0.0,
        }
    };
    let movable: Vec<usize> = (0..n).filter(|&a| chain_of[a] == usize::MAX).collect();
    if n == 0 || nets.is_empty() {
        return Ok(Placement { at, m20k_at });
    }
    // Wire length plus a cost per LAB in use: wire length alone doesn't
    // mind ALMs spread thinly between their nets' ends (the CPU's 2,127
    // ALMs took 522 LABs without it).
    const LAB_COST: f64 = 1.5;
    let mut ln = vec![0u32; labs.len()];
    for &(l, _) in &at {
        if l != usize::MAX {
            ln[l] += 1;
        }
    }
    let lab_term = |ln: &[u32]| LAB_COST * ln.iter().filter(|&&c| c > 0).count() as f64;
    let mut wl: f64 = (0..nets.len()).map(|i| net_cost(&at, i)).sum::<f64>() + lab_term(&ln);
    let mut tcost: f64 = if tc.is_some() { (0..tc.unwrap().conns.len()).map(|i| conn_cost(&at, i)).sum() } else { 0.0 };
    // Normalise the two terms by their starting values (VPR's
    // timing-driven cost).
    let lambda = tc.map_or(0.0, |t| t.lambda);
    let (wl_norm, t_norm) = (1.0 / wl.max(1.0), 1.0 / tcost.max(1e-9));
    let total = |wl: f64, t: f64| (1.0 - lambda) * wl * wl_norm + lambda * t * t_norm;
    let (gx, gy) = (labs.iter().map(|l| l.x).max().unwrap() as i64, labs.iter().map(|l| l.y).max().unwrap() as i64);
    let mut rlim = gx.max(gy) as f64;
    let mut moves_tried: Vec<usize> = Vec::new();
    // One move: an ALM (or its chain) to a random slot within rlim, swapping
    // with what is there. Returns the cost delta if accepted.
    let try_move = |at: &mut Vec<(usize, u8)>,
                        occ: &mut HashMap<(usize, u8), usize>,
                        uses: &mut Vec<LabUse>,
                        rng: &mut Rng,
                        temp: f64,
                        rlim: f64,
                        wl: &mut f64,
                        tcost: &mut f64,
                        ln: &mut Vec<u32>|
     -> Option<bool> {
        let a = if !p.chains.is_empty() && rng.below(8) == 0 {
            p.chains[rng.below(p.chains.len())][0]
        } else {
            *movable.get(rng.below(movable.len().max(1)))?
        };
        let r = rlim.max(1.0) as i64;
        let (l0, k0) = at[a];
        let tx = labs[l0].x as i64 + rng.below(2 * r as usize + 1) as i64 - r;
        let ty = labs[l0].y as i64 + rng.below(2 * r as usize + 1) as i64 - r;
        let &tl = lab_at.get(&(tx.clamp(0, 255) as u32, ty.clamp(0, 255) as u32))?;
        // The ALMs that move, old and new slots.
        let mut moved: Vec<(usize, (usize, u8), (usize, u8))> = Vec::new();
        if chain_of[a] != usize::MAX {
            let c = &p.chains[chain_of[a]];
            let k0 = 0;
            for (j, &m) in c.iter().enumerate() {
                let l = *lab_at.get(&(labs[tl].x, labs[tl].y.checked_sub(((k0 + j) / 10) as u32)?))?;
                let s = (l, ((k0 + j) % 10) as u8);
                if occ.get(&s).is_some_and(|o| !c.contains(o)) {
                    return None;
                }
                moved.push((m, at[m], s));
            }
        } else {
            let tk = rng.below(10) as u8;
            if (tl, tk) == (l0, k0) {
                return None;
            }
            moved.push((a, (l0, k0), (tl, tk)));
            if let Some(&o) = occ.get(&(tl, tk)) {
                if chain_of[o] != usize::MAX {
                    return None;
                }
                moved.push((o, (tl, tk), (l0, k0)));
            }
        }
        // LAB control lines.
        let mut trial: HashMap<usize, LabUse> = HashMap::default();
        for &(m, from, to) in &moved {
            trial.entry(from.0).or_insert_with(|| uses[from.0].clone()).add(&ctls[m], -1);
            trial.entry(to.0).or_insert_with(|| uses[to.0].clone()).add(&ctls[m], 1);
        }
        if !trial.values().all(LabUse::ok) {
            return None;
        }
        let mut touched: Vec<usize> = moved.iter().flat_map(|&(m, ..)| alm_nets[m].iter().copied()).collect();
        touched.sort();
        touched.dedup();
        let mut ttouched: Vec<usize> = moved.iter().flat_map(|&(m, ..)| alm_conns[m].iter().copied()).collect();
        ttouched.sort();
        ttouched.dedup();
        let before: f64 = touched.iter().map(|&i| net_cost(at, i)).sum();
        let tbefore: f64 = ttouched.iter().map(|&i| conn_cost(at, i)).sum();
        for &(m, _, to) in &moved {
            at[m] = to;
        }
        // LABs emptied and newly used.
        let mut dl: HashMap<usize, i32> = HashMap::default();
        for &(_, from, to) in &moved {
            *dl.entry(from.0).or_default() -= 1;
            *dl.entry(to.0).or_default() += 1;
        }
        let lab_delta: f64 = dl
            .iter()
            .map(|(&l, &d)| {
                let (o, n) = (ln[l] as i32, ln[l] as i32 + d);
                LAB_COST * ((n > 0) as i32 - (o > 0) as i32) as f64
            })
            .sum();
        let after: f64 = touched.iter().map(|&i| net_cost(at, i)).sum::<f64>() + lab_delta;
        let tafter: f64 = ttouched.iter().map(|&i| conn_cost(at, i)).sum();
        let delta = total(*wl + after - before, *tcost + tafter - tbefore) - total(*wl, *tcost);
        if delta <= 0.0 || rng.unit() < (-delta / temp).exp() {
            for &(_, from, _) in &moved {
                occ.remove(&from);
            }
            for &(m, _, to) in &moved {
                occ.insert(to, m);
            }
            for (l, u) in trial {
                uses[l] = u;
            }
            *wl += after - before;
            *tcost += tafter - tbefore;
            for (l, d) in dl {
                ln[l] = (ln[l] as i32 + d) as u32;
            }
            Some(true)
        } else {
            for &(m, from, _) in &moved {
                at[m] = from;
            }
            Some(false)
        }
    };
    // Starting temperature: 20 standard deviations of random moves' deltas.
    let mut deltas = Vec::new();
    for _ in 0..(n.min(500)).max(50) {
        let before = total(wl, tcost);
        if try_move(&mut at, &mut occ, &mut uses, &mut rng, f64::INFINITY, rlim, &mut wl, &mut tcost, &mut ln).is_some() {
            deltas.push(total(wl, tcost) - before);
        }
    }
    let mean = deltas.iter().sum::<f64>() / deltas.len().max(1) as f64;
    let sd = (deltas.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / deltas.len().max(1) as f64).sqrt();
    let mut temp = 20.0 * sd.max(1e-6);
    let inner = ((effort * (n as f64).powf(4.0 / 3.0)) as usize).max(200);
    let nets_n = nets.len().max(1) as f64;
    let mut rounds = 0;
    loop {
        let (mut acc, mut tried) = (0usize, 0usize);
        for _ in 0..inner {
            match try_move(&mut at, &mut occ, &mut uses, &mut rng, temp, rlim, &mut wl, &mut tcost, &mut ln) {
                Some(true) => {
                    acc += 1;
                    tried += 1
                }
                Some(false) => tried += 1,
                None => {}
            }
        }
        let rate = acc as f64 / tried.max(1) as f64;
        moves_tried.push(tried);
        let alpha = if rate > 0.96 {
            0.5
        } else if rate > 0.8 {
            0.9
        } else if rate > 0.15 {
            0.95
        } else {
            0.8
        };
        temp *= alpha;
        rlim = (rlim * (1.0 - 0.44 + rate)).clamp(1.0, gx.max(gy) as f64);
        rounds += 1;
        // Recompute from scratch now and then: the running sums drift.
        if rounds % 10 == 0 {
            wl = (0..nets.len()).map(|i| net_cost(&at, i)).sum::<f64>() + lab_term(&ln);
            if tc.is_some() {
                tcost = (0..tc.unwrap().conns.len()).map(|i| conn_cost(&at, i)).sum();
            }
        }
        if temp < 0.005 * total(wl, tcost) / nets_n || rounds > 400 {
            break;
        }
    }
    // A last greedy pass at zero temperature.
    for _ in 0..inner {
        try_move(&mut at, &mut occ, &mut uses, &mut rng, 1e-12, 3.0, &mut wl, &mut tcost, &mut ln);
    }
    let labs_used: std::collections::BTreeSet<usize> = at.iter().map(|a| a.0).collect();
    eprintln!(
        "place: {n} ALMs ({} in {} chains) in {} LABs, wire length {:.0}, {rounds} temperatures x {inner} moves",
        p.chains.iter().map(Vec::len).sum::<usize>(),
        p.chains.len(),
        labs_used.len(),
        (0..nets.len()).map(|i| net_cost(&at, i)).sum::<f64>()
    );
    Ok(Placement { at, m20k_at })
}
