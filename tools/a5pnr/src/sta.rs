//! Static timing of a packed design with the delay model (README (u), (z)):
//! a graph from flop outputs, input ports and M20K read data, through
//! routed (or estimated) connections, LUTs and carry chains, to flop and
//! M20K inputs, clock enables, synchronous clears and output ports. Arrival
//! times forward, required times backward against the worst arrival, and
//! per connection a slack and a criticality for timing-driven placement and
//! routing. One clock domain at a time; paths between domains aren't
//! analysed. No clock skew and no hold: it is the model's view, not
//! sign-off.

use std::collections::HashMap;

use crate::netlist::{Cell, Netlist};
use crate::pack::{Alm, End, Logic, Packed, Slot, Src, ARITH_IN, LEIM_POS, LUT_IN};

/// M20K read data comes out 1.43 ns after the clock (mm's report_path).
pub const M20K_TCO: f32 = 1.429;
/// An M20K input's setup time [inf: not fitted; a LAB flop's tsu plus a
/// margin until an M20K path is in the fit].
pub const M20K_TSU: f32 = 0.25;

#[derive(Clone, Copy)]
enum Arc {
    Fixed(f32),
    /// A routed connection, by index into `Sta::conns`.
    Conn(usize),
}

pub struct Sta {
    /// Connections: (net, sink end).
    pub conns: Vec<(u32, End)>,
    pub conn_of: HashMap<(u32, End), usize>,
    n: usize,
    edges: Vec<(u32, u32, Arc)>,
    /// Launch: node, time at the node (tco), clock net.
    starts: Vec<(u32, f32, u32)>,
    /// Capture: node, extra time (setup), clock net (None: an output port),
    /// and what it is.
    ends: Vec<(u32, f32, Option<u32>, String)>,
    /// Per node, the clock domain its arrival belongs to.
    clock_of_start: HashMap<u32, u32>,
}

pub struct Report {
    /// Worst arrival at a capture point, and where.
    pub worst: f32,
    pub at: String,
    /// Per connection: criticality in 0..1.
    pub crit: Vec<f32>,
    /// Per connection: slack against the worst path.
    pub slack: Vec<f32>,
}

fn ff_clock(nl: &Netlist, ff: usize) -> u32 {
    match &nl.cells[ff] {
        Cell::Ff(f) => f.clk,
        _ => unreachable!(),
    }
}

impl Sta {
    pub fn new(nl: &Netlist, p: &Packed, d: &HashMap<String, f32>) -> Sta {
        let get = |k: &str| d.get(k).copied().unwrap_or(0.0);
        let mut ids: HashMap<(u8, usize, u32), u32> = HashMap::new();
        let mut n = 0u32;
        let mut id = |k: (u8, usize, u32)| -> u32 {
            *ids.entry(k).or_insert_with(|| {
                n += 1;
                n - 1
            })
        };
        // Node keys: 0 net source, 1 ALM pin, 2 LUT out, 3 sum h, 4 cout h,
        // 5 capture point.
        let mut edges = Vec::new();
        let mut conns = Vec::new();
        let mut conn_of = HashMap::new();
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        let mut clock_of_start = HashMap::new();
        let clocks: std::collections::HashSet<u32> = p.clocks.iter().copied().collect();
        // Chain predecessor of each arithmetic half.
        let mut prev_half: HashMap<(usize, u8), (usize, u8)> = HashMap::new();
        for c in &p.chains {
            let mut last: Option<(usize, u8)> = None;
            for &a in c {
                if let Logic::Arith { halves } = &p.alms[a].logic {
                    for h in 0..2u8 {
                        if halves[h as usize].is_some() {
                            if let Some(l) = last {
                                prev_half.insert((a, h), l);
                            }
                            last = Some((a, h));
                        }
                    }
                }
            }
        }
        for (&net, (drv, sinks)) in &p.nets {
            if clocks.contains(&net) {
                continue;
            }
            let src = id((0, 0, net));
            match *drv {
                End::Out { alm, slot } => match p.alms[alm].slots[slot as usize] {
                    Some(Slot::Ff { ff, .. }) => {
                        starts.push((src, get("tco"), ff_clock(nl, ff)));
                    }
                    Some(Slot::Wire { src: Src::Lut }) => edges.push((id((2, alm, 0)), src, Arc::Fixed(0.0))),
                    Some(Slot::Wire { src: Src::Sum }) => edges.push((id((3, alm, slot as u32 / 2)), src, Arc::Fixed(0.0))),
                    _ => {}
                },
                End::Port(_) | End::Pll(_) => starts.push((src, 0.0, u32::MAX)),
                End::M20k { m, .. } => {
                    let Cell::M20k(c) = &nl.cells[p.m20ks[m]] else { unreachable!() };
                    starts.push((src, M20K_TCO, c.clk));
                }
                _ => {}
            }
            for e in sinks {
                let ci = conns.len();
                conns.push((net, *e));
                conn_of.insert((net, *e), ci);
                let dst = match *e {
                    End::In { alm, pin } => id((1, alm, pin as u32)),
                    End::Ctl { alm, ctl } => {
                        let x = id((5, conns.len(), 0));
                        let clk = p.alms[alm].ctls(nl).iter().find_map(|c| if let crate::pack::Ctl::Clock(k) = c { Some(*k) } else { None });
                        if matches!(ctl, crate::pack::Ctl::Ce(_) | crate::pack::Ctl::Sclr(_)) {
                            ends.push((x, get("tsu"), clk, format!("{} at ALM {alm}", nl.nets[net as usize])));
                        }
                        x
                    }
                    End::M20k { m, .. } => {
                        let x = id((5, conns.len(), 0));
                        let Cell::M20k(c) = &nl.cells[p.m20ks[m]] else { unreachable!() };
                        ends.push((x, M20K_TSU, Some(c.clk), format!("M20K {m}")));
                        x
                    }
                    End::Port(pi) => {
                        let x = id((5, conns.len(), 0));
                        ends.push((x, 0.0, None, format!("output {}", nl.ports[pi].name)));
                        x
                    }
                    End::Out { .. } | End::Pll(_) => continue,
                };
                edges.push((src, dst, Arc::Conn(ci)));
            }
        }
        for (a, alm) in p.alms.iter().enumerate() {
            match &alm.logic {
                Logic::Lut { cell, pins } => {
                    let Cell::Lut { ins, .. } = &nl.cells[*cell] else { unreachable!() };
                    for k in 0..ins.len() {
                        let pos = pins[k] as usize;
                        let letter = b"abcdef"[pos] as char;
                        edges.push((id((1, a, LUT_IN[pos] as u32)), id((2, a, 0)), Arc::Fixed(get(&format!("lut:data{letter}")))));
                    }
                }
                Logic::Arith { halves } => {
                    for h in 0..2usize {
                        if halves[h].is_none() {
                            continue;
                        }
                        let Cell::Arith { c, d, .. } = &nl.cells[halves[h].unwrap()] else { unreachable!() };
                        for (k, (letter, net)) in [("c", c), ("d", d)].into_iter().enumerate() {
                            if net.is_some() {
                                let pin = id((1, a, ARITH_IN[h][k] as u32));
                                edges.push((pin, id((3, a, h as u32)), Arc::Fixed(get(&format!("arith:data{letter}>sumout")))));
                                edges.push((pin, id((4, a, h as u32)), Arc::Fixed(get(&format!("arith:data{letter}>cout")))));
                            }
                        }
                        if let Some(&(pa, ph)) = prev_half.get(&(a, h as u8)) {
                            let cin = id((4, pa, ph as u32));
                            edges.push((cin, id((3, a, h as u32)), Arc::Fixed(get("arith:cin>sumout"))));
                            edges.push((cin, id((4, a, h as u32)), Arc::Fixed(get("arith:cin>cout"))));
                        }
                    }
                }
                Logic::None => {}
            }
            for s in 0..4usize {
                let Some(Slot::Ff { ff, src }) = alm.slots[s] else { continue };
                let from = match src {
                    Src::Lut => id((2, a, 0)),
                    Src::Sum => id((3, a, s as u32 / 2)),
                    Src::Leim => id((1, a, LUT_IN[LEIM_POS[s / 2] as usize] as u32)),
                };
                let x = id((5, usize::MAX - a * 4 - s, 1));
                edges.push((from, x, Arc::Fixed(get("reg_d"))));
                let Cell::Ff(f) = &nl.cells[ff] else { unreachable!() };
                ends.push((x, get("tsu"), Some(f.clk), format!("{} (ALM {a} slot {s})", nl.nets[f.q as usize])));
            }
        }
        for &(s, _, clk) in &starts {
            clock_of_start.insert(s, clk);
        }
        Sta { conns, conn_of, n: n as usize, edges, starts, ends, clock_of_start }
    }

    /// Analyse with per-connection delays.
    pub fn analyse(&self, conn_delay: &[f32]) -> Report {
        let delay = |a: &Arc| match a {
            Arc::Fixed(d) => *d,
            Arc::Conn(c) => conn_delay[*c],
        };
        // Topological order (Kahn).
        let mut indeg = vec![0u32; self.n];
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); self.n];
        for (i, (u, v, _)) in self.edges.iter().enumerate() {
            indeg[*v as usize] += 1;
            out[*u as usize].push(i);
        }
        let mut order = Vec::with_capacity(self.n);
        let mut stack: Vec<u32> = (0..self.n as u32).filter(|&v| indeg[v as usize] == 0).collect();
        while let Some(u) = stack.pop() {
            order.push(u);
            for &e in &out[u as usize] {
                let v = self.edges[e].1;
                indeg[v as usize] -= 1;
                if indeg[v as usize] == 0 {
                    stack.push(v);
                }
            }
        }
        // Arrival, and the launching clock (one per node: the latest's).
        let mut arr = vec![f32::NEG_INFINITY; self.n];
        let mut clk = vec![u32::MAX; self.n];
        for &(s, t, c) in &self.starts {
            arr[s as usize] = t;
            clk[s as usize] = c;
        }
        for &u in &order {
            if arr[u as usize] == f32::NEG_INFINITY {
                continue;
            }
            for &e in &out[u as usize] {
                let (_, v, a) = &self.edges[e];
                let t = arr[u as usize] + delay(a);
                if t > arr[*v as usize] {
                    arr[*v as usize] = t;
                    clk[*v as usize] = clk[u as usize];
                }
            }
        }
        // Capture: paths launched by another clock than the capturing one
        // (ports launch on any) aren't timed.
        let mut worst = (0.0f32, String::new());
        let mut req = vec![f32::INFINITY; self.n];
        let mut timed = Vec::new();
        for (x, extra, c, what) in &self.ends {
            let a = arr[*x as usize];
            if a == f32::NEG_INFINITY {
                continue;
            }
            let launched = clk[*x as usize];
            if c.is_some_and(|c| launched != u32::MAX && launched != c) {
                continue;
            }
            timed.push((*x, *extra));
            if a + extra > worst.0 {
                worst = (a + extra, what.clone());
            }
        }
        let dmax = worst.0.max(1e-3);
        for (x, extra) in timed {
            req[x as usize] = req[x as usize].min(dmax - extra);
        }
        for &u in order.iter().rev() {
            for &e in &out[u as usize] {
                let (_, v, a) = &self.edges[e];
                let r = req[*v as usize] - delay(a);
                if r < req[u as usize] {
                    req[u as usize] = r;
                }
            }
        }
        let mut slack = vec![dmax; self.conns.len()];
        let mut crit = vec![0.0f32; self.conns.len()];
        for (u, v, a) in &self.edges {
            if let Arc::Conn(c) = a {
                if arr[*u as usize] == f32::NEG_INFINITY || req[*v as usize] == f32::INFINITY {
                    continue;
                }
                let s = req[*v as usize] - arr[*u as usize] - conn_delay[*c];
                slack[*c] = s;
                crit[*c] = (1.0 - s / dmax).clamp(0.0, 1.0);
            }
        }
        let _ = &self.clock_of_start;
        Report { worst: worst.0, at: worst.1, crit, slack }
    }
}

/// Unused ALM count by kind, for reports.
pub fn alm_kinds(p: &Packed) -> (usize, usize, usize) {
    let (mut lut, mut arith, mut ffonly) = (0, 0, 0);
    for a in &p.alms {
        match a.logic {
            Logic::Lut { .. } => lut += 1,
            Logic::Arith { .. } => arith += 1,
            Logic::None => ffonly += 1,
        }
    }
    let _: Option<&Alm> = None;
    (lut, arith, ffonly)
}
