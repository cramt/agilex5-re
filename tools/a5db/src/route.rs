//! Routed nets → CRAM bits through the graph join, and the check against a
//! Quartus-routed `.sof` (README (k)).

use std::collections::{BTreeMap, HashMap, HashSet};

use rustc_hash::FxHashMap;

use anyhow::{Context, Result};

use crate::db::{BitPos, Db};
use crate::graph::{Graph, Join, Miss};

/// Fitter element types as DYGR names them: the fitter drops DYGR's
/// underscores (`HVIOWRX1RGMIIWRCONNRE` is `HVIOWR_X1_RGMII_WR_CONN_RE`).
pub fn type_aliases(g: &Graph) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for n in &g.names {
        if let Some((t, _)) = n.split_once(':') {
            if t.contains('_') && !out.contains_key(&t.replace('_', "")) {
                out.insert(t.replace('_', ""), t.to_owned());
            }
        }
    }
    out
}

/// A fitter routing-node name (`R0_X120_Y48_N0_I27`) as DYGR names it
/// (`H0:X120Y48Z0I27`); other names pass through.
pub fn dygr_name(fit: &str, aliases: &HashMap<String, String>) -> String {
    let Some((head, i)) = fit.rsplit_once("_I") else { return fit.to_owned() };
    let parse = || -> Option<String> {
        let (head, n) = head.rsplit_once("_N")?;
        let (head, y) = head.rsplit_once("_Y")?;
        let (t, x) = head.rsplit_once("_X")?;
        for v in [i, n, y, x] {
            v.parse::<u32>().ok()?;
        }
        let t = match t {
            "LOCAL_INTERCONNECT" => "LL".to_owned(),
            "BLOCK_INPUT_MUX_PASSTHROUGH" => "LEIM".to_owned(),
            "BLOCK_INPUT_MUX_PASSTHROUGH_WB" => "LEIM_WB".to_owned(),
            _ => match (t.as_bytes()[0], t[1..].parse::<u32>()) {
                (b'R', Ok(k)) => format!("H{k}"),
                (b'C', Ok(k)) => format!("V{k}"),
                _ => aliases.get(t).cloned().unwrap_or_else(|| t.to_owned()),
            },
        };
        Some(format!("{t}:X{x}Y{y}Z{n}I{i}"))
    };
    parse().unwrap_or_else(|| fit.to_owned())
}

/// Routing paths from `tools/rgdump/routes.tcl` output ("C …", "R <hop>
/// <fanout>"…, "E"), or one path of DYGR/fitter node names per line. Hops
/// that are netlist ports (contain '/') are dropped.
pub fn read_routes(path: &str, aliases: &HashMap<String, String>) -> Result<Vec<Vec<String>>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let mut out = Vec::new();
    let mut cur: Option<Vec<String>> = None;
    for l in text.lines() {
        if l.starts_with("C ") {
            cur = Some(Vec::new());
        } else if let Some(h) = l.strip_prefix("R ") {
            let name = h.split_whitespace().next().unwrap_or("");
            if let Some(c) = cur.as_mut() {
                if !name.contains('/') {
                    c.push(dygr_name(name, aliases));
                }
            }
        } else if l == "E" {
            out.extend(cur.take());
        } else if !l.starts_with("X ") && !l.starts_with("A ") && !l.trim().is_empty() && cur.is_none() {
            out.push(l.split_whitespace().map(|n| dygr_name(n, aliases)).collect());
        }
    }
    Ok(out)
}

/// Every image bit some edge configuration writes, with its polarity and
/// the first (uid id, config id) that writes it, and the misses of edges
/// that resolve to no bits.
pub struct Universe {
    /// (polarity, idle value, uid id, config id)
    pub bits: FxHashMap<(usize, u8), (bool, bool, u32, u32)>,
    pub misses: BTreeMap<Miss, usize>,
    pub edges: usize,
    pub resolved: usize,
    pub pairs: usize,
}

pub fn universe(join: &mut Join, graph: &Graph) -> Result<Universe> {
    let mut seen: FxHashMap<(u32, u32), usize> = FxHashMap::default();
    for e in 0..graph.dst.len() {
        if graph.cfg[e] != 0 {
            *seen.entry((graph.uid[e], graph.cfg[e])).or_default() += 1;
        }
    }
    let mut u = Universe { bits: FxHashMap::default(), misses: BTreeMap::new(), edges: 0, resolved: 0, pairs: seen.len() };
    for (&(uid, cfg), &n) in &seen {
        u.edges += n;
        match join.pair(uid, cfg)? {
            Some(Ok(_)) => {
                u.resolved += n;
                if let Some(Ok(idle)) = join.pair_idle(uid, cfg)? {
                    for (p, v) in idle {
                        u.bits.entry((p.byte, p.bit)).or_insert((p.equal, v, uid, cfg));
                    }
                }
            }
            Some(Err(m)) => *u.misses.entry(m).or_default() += n,
            None => {}
        }
    }
    Ok(u)
}

pub struct Applied {
    pub hops: usize,
    pub no_edge: Vec<(String, String)>,
    pub unknown_node: HashSet<String>,
    pub configured: usize,
    pub pass_through: usize,
    pub misses: BTreeMap<Miss, usize>,
}

/// Write the bits of every edge along the paths into `img`.
pub fn apply(join: &mut Join, graph: &Graph, idx: &HashMap<&str, u32>, paths: &[Vec<String>], img: &mut [u8]) -> Result<Applied> {
    let mut a = Applied { hops: 0, no_edge: Vec::new(), unknown_node: HashSet::new(), configured: 0, pass_through: 0, misses: BTreeMap::new() };
    let mut done: HashSet<usize> = HashSet::new();
    for path in paths {
        for w in path.windows(2) {
            a.hops += 1;
            let (Some(&s), Some(&d)) = (idx.get(w[0].as_str()), idx.get(w[1].as_str())) else {
                for n in w {
                    if !idx.contains_key(n.as_str()) {
                        a.unknown_node.insert(n.clone());
                    }
                }
                continue;
            };
            let Some(e) = graph.edge(s, d) else {
                a.no_edge.push((w[0].clone(), w[1].clone()));
                continue;
            };
            if !done.insert(e) {
                continue;
            }
            match join.edge(e)? {
                Some(Ok(ws)) => {
                    a.configured += 1;
                    for (p, v) in ws {
                        p.set(img, v);
                    }
                }
                Some(Err(m)) if m.is_pass_through() => a.pass_through += 1,
                Some(Err(m)) => *a.misses.entry(m).or_default() += 1,
                None => {}
            }
        }
    }
    Ok(a)
}

/// Set every routing bit to its mux's idle state.
pub fn clear(u: &Universe, img: &mut [u8]) {
    for (&(byte, bit), &(equal, idle, ..)) in &u.bits {
        BitPos { byte, bit, equal }.set(img, idle);
    }
}

/// A routing bit's block and mux, from the first configuration writing it.
pub fn bit_name(u: &Universe, graph: &Graph, at: (usize, u8)) -> String {
    let (_, _, uid, cfg) = u.bits[&at];
    format!("{} {}", graph.uids[uid as usize], graph.cfgs[cfg as usize].split(' ').next().unwrap_or(""))
}

pub fn open(dbdir: &str, gdir: &str) -> Result<(Db, Graph)> {
    let t = std::time::Instant::now();
    let g = Graph::load(std::path::Path::new(gdir))?;
    eprintln!("graph: {} nodes, {} edges, {:.0} s", g.names.iter().filter(|n| !n.is_empty()).count(), g.dst.len(), t.elapsed().as_secs_f64());
    Ok((Db::open(dbdir)?, g))
}
