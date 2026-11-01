//! Pin ↔ routing-graph node map (README (t)): where a pin's input leaves the
//! I/O block for fabric routing, and where fabric routing enters a pin's
//! output.
//!
//! A pad's graph nodes are `<type>:X<x>Y<y>Z<sub_loc>I<i>` at its fitter
//! location (`io/pins.tsv`). Its input buffer (`IO_INPUT_BUFFER_OUTPUT`) and
//! output buffer (`IO_OUTPUT_BUFFER_INPUT`) sit next to those nodes. Inside
//! the I/O block every edge is unconfigured: the template's bits (README (p))
//! set the muxes there, not the graph. So the pin's side of a net is the set
//! of nodes reachable from the buffer through unconfigured edges, and its
//! boundary is where the first configured edge (a fabric routing mux, or a
//! clock-network rule) starts or ends.

use std::collections::{BTreeMap, HashMap, VecDeque};

use anyhow::{Context, Result};

use crate::db::Db;
use crate::graph::{clock_rule, parse_node, Graph};

pub struct Ball {
    pub ball: String,
    pub uid: String,
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

pub fn balls(db: &Db) -> Result<Vec<Ball>> {
    let text = std::fs::read_to_string(db.dir.join("io/pins.tsv")).context("reading io/pins.tsv")?;
    let mut out = Vec::new();
    for l in text.lines().filter(|l| !l.starts_with('#') && !l.starts_with("ball\t") && !l.is_empty()) {
        let f: Vec<&str> = l.split('\t').collect();
        anyhow::ensure!(f.len() >= 7, "io/pins.tsv: {l:?} has no x, y, sub_loc");
        out.push(Ball { ball: f[0].into(), uid: f[1].into(), x: f[4].parse()?, y: f[5].parse()?, z: f[6].parse()? });
    }
    Ok(out)
}

/// Reverse CSR of the graph: fanin edges of each node.
pub struct Rev {
    first: Vec<u32>,
    pub edge: Vec<u32>,
    pub src: Vec<u32>,
}

impl Rev {
    pub fn new(g: &Graph) -> Rev {
        let n = g.names.len();
        let mut count = vec![0u32; n + 1];
        for &d in &g.dst {
            count[d as usize + 1] += 1;
        }
        for i in 0..n {
            count[i + 1] += count[i];
        }
        let first = count.clone();
        let mut fill = count;
        let (mut edge, mut src) = (vec![0u32; g.dst.len()], vec![0u32; g.dst.len()]);
        for s in 0..n as u32 {
            for e in g.fanout(s) {
                let d = g.dst[e] as usize;
                edge[fill[d] as usize] = e as u32;
                src[fill[d] as usize] = s;
                fill[d] += 1;
            }
        }
        Rev { first, edge, src }
    }

    pub fn fanin(&self, n: u32) -> std::ops::Range<usize> {
        self.first[n as usize] as usize..self.first[n as usize + 1] as usize
    }
}

/// Fabric routing node types: a pin's side of a net never runs through them.
pub fn is_fabric(name: &str) -> bool {
    let t = name.split(':').next().unwrap_or("");
    matches!(t, "LL" | "LEIM" | "LEIM_WB" | "LAB_RE" | "HIOB" | "VIOB")
        || ((t.starts_with('H') || t.starts_with('V')) && t.len() > 1 && t[1..].parse::<u32>().is_ok())
}

/// An edge the router sets: a mux in a routing tile (`cw_rt_*`) or the clock
/// network (`cw_cc_*`, or a clock rule). Edges inside I/O blocks have no
/// edge API (the pin template sets them), and the redundancy muxes
/// (`cw_rd_*`, no bit map) pass their data input in the idle state, so the
/// search goes through both.
pub fn routing(g: &Graph, e: usize) -> bool {
    if g.cfg[e] == 0 {
        return clock_rule(g, e).is_some();
    }
    let u = &g.uids[g.uid[e] as usize];
    u.starts_with("cw_rt_") || u.starts_with("cw_cc_")
}

/// One boundary of a pin: the node, how many unconfigured hops from the
/// buffer, and the path's nodes (buffer first for inputs, last for outputs).
pub struct Boundary {
    pub node: u32,
    pub depth: usize,
    pub path: Vec<u32>,
}

/// Nodes at (x, y, z), by scanning the names once.
pub fn nodes_at(g: &Graph, want: &[(u32, u32, u32)]) -> HashMap<(u32, u32, u32), Vec<u32>> {
    let set: std::collections::HashSet<_> = want.iter().copied().collect();
    let mut out: HashMap<_, Vec<u32>> = HashMap::new();
    for (i, n) in g.names.iter().enumerate() {
        if let Some((_, x, y, z, _)) = parse_node(n) {
            if set.contains(&(x, y, z)) {
                out.entry((x, y, z)).or_default().push(i as u32);
            }
        }
    }
    out
}

fn ty(g: &Graph, n: u32) -> &str {
    g.names[n as usize].split(':').next().unwrap_or("")
}

/// The pad's input buffer outputs and output buffer data inputs.
pub fn buffers(g: &Graph, rev: &Rev, pad: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut ins = Vec::new();
    let mut outs = Vec::new();
    let padset: std::collections::HashSet<u32> = pad.iter().copied().collect();
    for &p in pad {
        if ty(g, p) == "IO_INPUT_BUFFER_OUTPUT" {
            ins.push(p);
        }
        if ty(g, p) == "IO_OUTPUT_BUFFER_INPUT" {
            outs.push(p);
        }
        for f in rev.fanin(p) {
            let s = rev.src[f];
            if ty(g, s) == "IO_INPUT_BUFFER_OUTPUT" && !padset.contains(&s) {
                ins.push(s);
            }
        }
        for e in g.fanout(p) {
            let d = g.dst[e];
            if ty(g, d) == "IO_OUTPUT_BUFFER_INPUT" && !padset.contains(&d) {
                outs.push(d);
            }
        }
    }
    ins.sort();
    ins.dedup();
    outs.sort();
    outs.dedup();
    (ins, outs)
}

/// Forward (inputs) or backward (outputs) search through unconfigured
/// edges; a node with a configured edge out of it (in, for outputs), or a
/// fabric node, is a boundary. `max` bounds the depth.
pub fn search(g: &Graph, rev: &Rev, start: &[u32], forward: bool, max: usize) -> Vec<Boundary> {
    let mut prev: HashMap<u32, u32> = HashMap::new();
    let mut depth: HashMap<u32, usize> = HashMap::new();
    let mut q = VecDeque::new();
    for &s in start {
        depth.insert(s, 0);
        q.push_back(s);
    }
    let mut out = BTreeMap::new();
    while let Some(n) = q.pop_front() {
        let d = depth[&n];
        let mut configured = false;
        let mut next = Vec::new();
        if forward {
            for e in g.fanout(n) {
                if routing(g, e) {
                    configured = true;
                } else {
                    next.push(g.dst[e]);
                }
            }
        } else {
            for f in rev.fanin(n) {
                let e = rev.edge[f] as usize;
                if routing(g, e) {
                    configured = true;
                } else {
                    next.push(rev.src[f]);
                }
            }
        }
        if configured || is_fabric(&g.names[n as usize]) {
            let mut path = vec![n];
            let mut c = n;
            while let Some(&p) = prev.get(&c) {
                path.push(p);
                c = p;
            }
            if forward {
                path.reverse();
            }
            out.insert(n, Boundary { node: n, depth: d, path });
            if is_fabric(&g.names[n as usize]) {
                continue;
            }
        }
        if d >= max {
            continue;
        }
        for m in next {
            if !depth.contains_key(&m) {
                depth.insert(m, d + 1);
                prev.insert(m, n);
                q.push_back(m);
            }
        }
    }
    out.into_values().collect()
}

/// A pin's boundary nodes by role: `in0`/`in1` (the input leaving through
/// input register mux 0 or 1), `clock` (into the clock network), `direct`
/// (an IO96 input path that bypasses both), `out` (output data), `oe`
/// (output enable) and `out2` (the HVIO buffer's third input) [README (t)].
pub fn roles(g: &Graph, rev: &Rev, pad: &[u32]) -> Vec<(&'static str, u32, usize)> {
    let (ins, outs) = buffers(g, rev, pad);
    let has = |b: &Boundary, s: &str| b.path.iter().any(|&n| g.names[n as usize].contains(s));
    let mut out = Vec::new();
    let mut data = Vec::new();
    for &s in &ins {
        for b in search(g, rev, &[s], true, 200) {
            if has(&b, "P2C_IOCLK_2TO1") || has(&b, "P2C_SSCLK_4TO1") || has(&b, "IOFBRADAPT_P2C_FRZ") {
                out.push(("clock", b.node, b.depth));
            } else if has(&b, "P2C_GPIO_FLYBY_LS") {
                out.push(("in0", b.node, b.depth));
            } else if let Some(&m) = b.path.iter().find(|&&n| ty(g, n) == "IOGPIO_REG_4TO1_MUX_RE") {
                data.push((parse_node(&g.names[m as usize]).map_or(0, |p| p.3), b.node, b.depth));
            } else {
                out.push(("direct", b.node, b.depth));
            }
        }
    }
    // An HVIO pin's two input register muxes are adjacent Z; the lower is
    // mux 0. On IO96 only mux 1 shows in the graph (mux 0 is the flyby).
    data.sort();
    let io96 = out.iter().any(|r| r.0 == "in0");
    for (k, (_, n, d)) in data.iter().enumerate() {
        out.push((if io96 || k == 1 { "in1" } else { "in0" }, *n, *d));
    }
    for &s in &outs {
        let role = match parse_node(&g.names[s as usize]).map(|p| p.4) {
            Some(0) => "out",
            Some(1) => "oe",
            _ => "out2",
        };
        for b in search(g, rev, &[s], false, 200) {
            out.push((role, b.node, b.depth));
        }
    }
    // pio_1_0 pads have two input buffers that reach the same boundaries.
    out.sort_by_key(|r| (r.0, r.1, r.2));
    out.dedup_by_key(|r| (r.0, r.1));
    out
}
