//! PathFinder routing over the graph (README (w)): every net is a tree from
//! its source node to its sinks, found by A* from the tree built so far;
//! nodes carry one net, and overused nodes get dearer each iteration until
//! no node is shared. Node costs are the delay model's per-type means
//! (tools/timing/fit.py, README (u)).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use anyhow::{bail, Result};

use crate::graph::{Graph, CLOCK, MUX, PASS};

pub struct Net {
    pub name: String,
    pub source: u32,
    pub sinks: Vec<u32>,
    pub clock: bool,
}

pub struct Routed {
    /// Per net: the tree's edges as (from, to).
    pub trees: Vec<Vec<(u32, u32)>>,
    pub iterations: usize,
}

/// Per node type, a base cost: the mean delay into that node type (ns).
pub fn base_costs(g: &Graph, delays: &HashMap<String, f32>) -> Vec<f32> {
    g.types
        .iter()
        .map(|t| delays.get(&format!("re:{t}")).copied().unwrap_or(0.05).max(0.005))
        .collect()
}

pub fn route(g: &Graph, nets: &[Net], base_by_type: &[f32], max_iter: usize) -> Result<Routed> {
    let n = g.len();
    let mut occ = vec![0u16; n];
    let mut hist = vec![0f32; n];
    let mut cost = vec![f32::INFINITY; n];
    let mut prev = vec![u32::MAX; n];
    let mut stamp = vec![0u32; n];
    let mut stamp_now = 0u32;
    let lab_re = g.type_id("LAB_RE");
    let mut trees: Vec<Vec<(u32, u32)>> = vec![Vec::new(); nets.len()];
    let mut pres = 0.5f32;
    for iter in 1..=max_iter {
        for (ni, net) in nets.iter().enumerate() {
            // Rip up.
            for &(_, d) in &trees[ni] {
                occ[d as usize] -= 1;
            }
            if !trees[ni].is_empty() {
                occ[net.source as usize] -= 1;
            }
            trees[ni].clear();
            occ[net.source as usize] += 1;
            let mut in_tree: Vec<u32> = vec![net.source];
            let mut sinks = net.sinks.clone();
            let (sx, sy) = (g.x[net.source as usize] as i32, g.y[net.source as usize] as i32);
            sinks.sort_by_key(|&s| (g.x[s as usize] as i32 - sx).abs() + (g.y[s as usize] as i32 - sy).abs());
            for &t in &sinks {
                if in_tree.contains(&t) {
                    continue;
                }
                stamp_now += 1;
                let (tx, ty) = (g.x[t as usize] as i32, g.y[t as usize] as i32);
                let h = |m: u32| 0.006 * ((g.x[m as usize] as i32 - tx).abs() + (g.y[m as usize] as i32 - ty).abs()) as f32;
                let mut heap = BinaryHeap::new();
                for &m in &in_tree {
                    stamp[m as usize] = stamp_now;
                    cost[m as usize] = 0.0;
                    prev[m as usize] = u32::MAX;
                    heap.push((Reverse(h(m).to_bits()), m));
                }
                let mut found = false;
                let mut expanded = 0usize;
                while let Some((Reverse(fb), m)) = heap.pop() {
                    if m == t {
                        found = true;
                        break;
                    }
                    let gm = cost[m as usize];
                    if f32::from_bits(fb) > gm + h(m) + 1e-6 {
                        continue;
                    }
                    expanded += 1;
                    if expanded > 4_000_000 {
                        break;
                    }
                    // LAB_RE pins are cell ports: never pass through one.
                    if Some(g.ty[m as usize]) == lab_re && m != net.source {
                        continue;
                    }
                    for e in g.fanout(m) {
                        let c = g.class[e];
                        let d = g.dst[e];
                        let ok = if net.clock { c == CLOCK || ((c == PASS || c == MUX) && !g.is_wire(d)) } else { c == MUX || c == PASS };
                        if !ok {
                            continue;
                        }
                        let du = d as usize;
                        let over = occ[du] as f32;
                        let nc = gm + (base_by_type[g.ty[du] as usize] + hist[du]) * (1.0 + pres * over);
                        if stamp[du] != stamp_now || nc < cost[du] {
                            stamp[du] = stamp_now;
                            cost[du] = nc;
                            prev[du] = m;
                            heap.push((Reverse((nc + h(d)).to_bits()), d));
                        }
                    }
                }
                if !found {
                    bail!("net {}: no path to {} (iteration {iter}, {expanded} nodes expanded)", net.name, g.name(t));
                }
                let mut m = t;
                while prev[m as usize] != u32::MAX {
                    let p = prev[m as usize];
                    trees[ni].push((p, m));
                    occ[m as usize] += 1;
                    in_tree.push(m);
                    m = p;
                }
            }
        }
        let over: Vec<usize> = (0..n).filter(|&k| occ[k] > 1).collect();
        eprintln!("route: iteration {iter}: {} overused nodes", over.len());
        if over.is_empty() {
            return Ok(Routed { trees, iterations: iter });
        }
        for &k in &over {
            hist[k] += 0.05 * (occ[k] - 1) as f32;
        }
        pres *= 1.8;
    }
    bail!("routing did not converge in {max_iter} iterations")
}
