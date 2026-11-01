//! a5pnr: a small placer and router for the Agilex 5 A5EB013B (README (w), (x)–(z)).
//!
//!   a5pnr <db> <graph> <delays.tsv> <netlist.json> <top> <pins.txt> <out design.txt> [options]
//!     --seed N       placement seed (1)
//!     --effort X     annealing moves per temperature, x N^(4/3) (1)
//!     --no-timing    wire length only, no timing-driven placement or routing
//!
//! Reads a yosys netlist (tools/a5pnr/synth.sh), packs it into ALMs, places
//! them by simulated annealing, routes every net with PathFinder over the
//! routing graph (`<graph>`: tools/rgdump output plus `a5db usable`'s
//! usable.bin) and writes an a5db design file; `a5db build` and `a5db rbf`
//! turn that into a bitstream. pins.txt: `<port> <ball> <3.3|1.8|1.1>` lines.
//! delays.tsv: tools/timing/fit.py's model (node costs, timing).

mod emit;
mod graph;
mod netlist;
mod pack;
mod place;
mod readback;
mod route;
mod sta;

use std::collections::HashMap;
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};

use netlist::Sig;
use pack::End;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if let [_, cmd, gdir, nodes @ ..] = args.as_slice() {
        if cmd == "readback" {
            // a5pnr readback <graph> <db> <extract.txt> <pins.txt> <top> <out.v>
            let [db, extract, pins, top, out] = nodes else { bail!("usage: a5pnr readback <graph> <db> <extract> <pins> <top> <out.v>") };
            let g = graph::Graph::load(std::path::Path::new(gdir))?;
            return readback::run(&g, db, extract, pins, top, out);
        }
        if cmd == "drivers" {
            // Debugging aid: for a decoded design (a5db extract), the source
            // of every reached node whose name starts with the prefix.
            let [db, extract, prefix] = nodes else { bail!("usage: a5pnr drivers <graph> <db> <extract> <type prefix>") };
            let g = graph::Graph::load(std::path::Path::new(gdir))?;
            return readback::drivers(&g, db, extract, prefix);
        }
        if cmd == "ambiguous" {
            // Debugging aid: node types with more than one pass-through fanin.
            let g = graph::Graph::load(std::path::Path::new(gdir))?;
            let mut n = vec![0u8; g.len()];
            for e in 0..g.dst.len() {
                if g.class[e] == graph::PASS {
                    let d = g.dst[e] as usize;
                    n[d] = n[d].saturating_add(1);
                }
            }
            let mut by: std::collections::BTreeMap<String, usize> = Default::default();
            for (k, &c) in n.iter().enumerate() {
                if c > 1 {
                    *by.entry(g.type_name(k as u32).to_owned()).or_default() += 1;
                }
            }
            for (t, c) in by {
                println!("{t}\t{c}");
            }
            return Ok(());
        }
        if cmd == "reach" {
            // Debugging aid: every path of up to 6 edges from a node to a
            // node whose name starts with the last argument.
            let [from @ .., stop] = nodes else { bail!("usage: a5pnr reach <graph> <node>... <type prefix>") };
            let g = graph::Graph::load(std::path::Path::new(gdir))?;
            for n in from {
                let m = g.node(n).with_context(|| format!("no node {n}"))?;
                let mut paths = vec![vec![m]];
                for _ in 0..6 {
                    let mut next = Vec::new();
                    for p in paths {
                        let last = *p.last().unwrap();
                        if g.name(last).starts_with(stop.as_str()) && p.len() > 1 {
                            println!("{}", p.iter().map(|&k| g.name(k)).collect::<Vec<_>>().join(" > "));
                            continue;
                        }
                        for e in g.fanout(last) {
                            let mut q = p.clone();
                            q.push(g.dst[e]);
                            next.push(q);
                        }
                    }
                    paths = next;
                }
            }
            return Ok(());
        }
        if cmd == "fanout" {
            // Debugging aid: each node's fanout with the edge classes.
            let g = graph::Graph::load(std::path::Path::new(gdir))?;
            for n in nodes {
                let m = g.node(n).with_context(|| format!("no node {n}"))?;
                for e in g.fanout(m) {
                    println!("{n}\t{}\t{}", g.name(g.dst[e]), g.class[e]);
                }
            }
            return Ok(());
        }
    }
    let [_, db, gdir, delays, json, top, pins, out, rest @ ..] = args.as_slice() else {
        bail!("usage: a5pnr <db> <graph> <delays.tsv> <netlist.json> <top> <pins.txt> <out design.txt> [--seed N] [--effort X] [--no-timing]");
    };
    let (mut seed, mut effort, mut timing_driven) = (1u64, 1.0f64, true);
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().context("--seed N")?.parse()?,
            "--effort" => effort = it.next().context("--effort X")?.parse()?,
            "--no-timing" => timing_driven = false,
            x => bail!("unknown option {x}"),
        }
    }
    let t0 = std::time::Instant::now();
    let nl = netlist::read(json, top)?;
    let packed = pack::pack(&nl)?;
    let (n_lut, n_arith, n_ff) = sta::alm_kinds(&packed);
    let n_ffs = nl.cells.iter().filter(|c| matches!(c, netlist::Cell::Ff(_))).count();
    eprintln!(
        "pack: {} cells into {} ALMs ({n_lut} LUT, {n_arith} arithmetic in {} chains, {n_ff} flops only), {n_ffs} flops, {} M20Ks, {} clocks",
        nl.cells.len(),
        packed.alms.len(),
        packed.chains.len(),
        packed.m20ks.len(),
        packed.clocks.len()
    );

    // Pins: port → (ball, std), and the ball's boundary nodes (README (t)).
    let mut pin_of: HashMap<String, (String, String)> = HashMap::new();
    for l in std::fs::read_to_string(pins).context("reading pins")?.lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() >= 3 && !f[0].starts_with('#') {
            pin_of.insert(f[0].to_owned(), (f[1].to_owned(), f[2].to_owned()));
        }
    }
    let mut nodes_of: HashMap<(String, String), String> = HashMap::new();
    let mut io96: HashMap<String, bool> = HashMap::new();
    for l in std::fs::read_to_string(format!("{db}/io/pin_nodes.tsv"))?.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 5 && !l.starts_with('#') && f[0] != "ball" {
            nodes_of.insert((f[0].to_owned(), f[2].to_owned()), f[3].to_owned());
            io96.insert(f[0].to_owned(), f[1].starts_with("pio"));
        }
    }
    let g = graph::Graph::load(std::path::Path::new(gdir))?;
    eprintln!("graph: {} nodes, {} edges, {:.0} s", g.len(), g.dst.len(), t0.elapsed().as_secs_f64());
    let mut port_node: HashMap<usize, u32> = HashMap::new();
    let mut pin_lines = Vec::new();
    for (pi, p) in nl.ports.iter().enumerate() {
        let Some((ball, std)) = pin_of.get(&p.name) else {
            if matches!(p.net, Sig::Net(n) if packed.nets.contains_key(&n)) {
                bail!("port {} has no pin in {pins}", p.name);
            }
            continue;
        };
        let is_clock = p.input && matches!(p.net, Sig::Net(n) if packed.clocks.contains(&n));
        let (role, usage) = if is_clock {
            ("clock", "clock_in")
        } else if p.input {
            (if io96.get(ball) == Some(&true) { "in0" } else { "in1" }, "in")
        } else {
            ("out", "out")
        };
        let node = nodes_of.get(&(ball.clone(), role.to_owned())).with_context(|| format!("{ball} has no {role} node"))?;
        port_node.insert(pi, g.node(node).with_context(|| format!("no node {node}"))?);
        pin_lines.push(format!("pin\t{ball}={usage}:{std}"));
    }
    let port_xy: HashMap<usize, (u32, u32)> = port_node.iter().map(|(&p, &n)| (p, (g.x[n as usize] as u32, g.y[n as usize] as u32))).collect();
    let labs = place::labs(db)?;
    let sites = place::m20k_sites(db)?;
    let mut delay: HashMap<String, f32> = HashMap::new();
    for l in std::fs::read_to_string(delays).context("reading delays")?.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() >= 3 {
            delay.insert(f[0].to_owned(), f[2].parse()?);
        }
    }
    let base = route::base_costs(&g, &delay);
    let sta = sta::Sta::new(&nl, &packed, &delay);

    // Placement: wire length first; then, timing-driven, again with the
    // connections' criticalities from that placement (README (z)).
    let t1 = std::time::Instant::now();
    let mut pl = place::place(&packed, &nl, &labs, &sites, &port_xy, seed, None, effort)?;
    let mut dtab = None;
    if timing_driven {
        let t = route::delay_table(&g, &base, &labs)?;
        let est = |pl: &place::Placement| -> Vec<f32> { sta.conns.iter().map(|(n, e)| t.conn(&packed, pl, &labs, &port_xy, packed.nets[n].0, *e)).collect() };
        let r0 = sta.analyse(&est(&pl));
        eprintln!("timing: wire-length placement, {:.3} ns estimated ({})", r0.worst, r0.at);
        let conns: Vec<(End, End, f64)> = sta.conns.iter().enumerate().map(|(i, (n, e))| (packed.nets[n].0, *e, (r0.crit[i] as f64).powi(8))).collect();
        let delay_fn = |dx: i64, dy: i64| t.lab(dx, dy) as f64;
        let tc = place::TimingCost { conns, delay: &delay_fn, lambda: 0.5 };
        pl = place::place(&packed, &nl, &labs, &sites, &port_xy, seed, Some(&tc), effort)?;
        let r1 = sta.analyse(&est(&pl));
        eprintln!("timing: timing-driven placement, {:.3} ns estimated ({})", r1.worst, r1.at);
        dtab = Some(t);
    }
    let place_s = t1.elapsed().as_secs_f64();
    let lines = emit::lab_lines(&packed, &nl, &pl);
    let lab_re = |l: usize, pin: u32| -> Result<u32> {
        let name = format!("LAB_RE:X{}Y{}Z0I{pin}", labs[l].x, labs[l].y);
        g.node(&name).with_context(|| format!("no node {name}"))
    };
    let node = |e: &End| -> Result<u32> {
        match *e {
            End::Out { alm, slot } => lab_re(pl.at[alm].0, 89 + 4 * pl.at[alm].1 as u32 + slot as u32),
            End::In { alm, pin } => lab_re(pl.at[alm].0, 8 * pl.at[alm].1 as u32 + pin as u32),
            End::Ctl { alm, ctl } => {
                let l = pl.at[alm].0;
                lab_re(l, emit::ctl_pin(&lines[&l], ctl))
            }
            End::Port(p) => port_node.get(&p).copied().with_context(|| format!("port {} has no node", nl.ports[p].name)),
            End::M20k { m, pin } => {
                let (x, y) = pl.m20k_at[m];
                let name = format!("MEDIUM_EAB_RE:X{x}Y{y}Z0I{pin}");
                g.node(&name).with_context(|| format!("no node {name}"))
            }
        }
    };
    // Nets as source and sink nodes; per sink the connections it serves.
    let mut rnets = Vec::new();
    let mut sink_conns: Vec<HashMap<u32, Vec<usize>>> = Vec::new();
    for (net, (d, sinks)) in &packed.nets {
        let mut by_node: HashMap<u32, Vec<usize>> = HashMap::new();
        for e in sinks {
            let n = node(e)?;
            let ci = sta.conn_of.get(&(*net, *e)).copied().unwrap_or(usize::MAX);
            by_node.entry(n).or_default().push(ci);
        }
        let mut ss: Vec<u32> = by_node.keys().copied().collect();
        ss.sort();
        rnets.push(route::Net { name: nl.nets[*net as usize].clone(), source: node(d)?, sinks: ss, clock: packed.clocks.contains(net) });
        sink_conns.push(by_node);
    }
    let order: Vec<usize> = {
        let mut o: Vec<usize> = (0..rnets.len()).collect();
        o.sort_by_key(|&i| (!rnets[i].clock, std::cmp::Reverse(rnets[i].sinks.len())));
        o
    };
    let rnets: Vec<route::Net> = order.iter().map(|&i| std::mem::replace(&mut rnets[i], route::Net::default())).collect();
    let sink_conns: Vec<HashMap<u32, Vec<usize>>> = order.iter().map(|&i| sink_conns[i].clone()).collect();
    let t2 = std::time::Instant::now();
    // Per sink criticality for the router, from the placement's estimate.
    let mut crit: Vec<Vec<f32>> = rnets.iter().map(|n| vec![0.0; n.sinks.len()]).collect();
    if let Some(t) = &dtab {
        let r = sta.analyse(&sta.conns.iter().map(|(n, e)| t.conn(&packed, &pl, &labs, &port_xy, packed.nets[n].0, *e)).collect::<Vec<_>>());
        for (ni, n) in rnets.iter().enumerate() {
            for (si, s) in n.sinks.iter().enumerate() {
                crit[ni][si] = sink_conns[ni][s].iter().filter(|&&c| c != usize::MAX).map(|&c| r.crit[c]).fold(0.0, f32::max);
            }
        }
    }
    let routed = route::route(&g, &rnets, &base, &crit, 80)?;
    let hops: usize = routed.trees.iter().map(|t| t.len()).sum();
    let route_s = t2.elapsed().as_secs_f64();
    eprintln!("route: {} nets, {hops} hops, {} iterations, {route_s:.1} s", rnets.len(), routed.iterations);

    // Timing of the routed design (README (u), (z)): each routed hop costs
    // its node type's mean, cells their arcs' means.
    let mut conn_delay = vec![0f32; sta.conns.len()];
    for (ni, tree) in routed.trees.iter().enumerate() {
        let parent: HashMap<u32, u32> = tree.iter().map(|&(a, b)| (b, a)).collect();
        for (s, cs) in &sink_conns[ni] {
            let mut d = 0.0f32;
            let mut m = *s;
            while let Some(&p) = parent.get(&m) {
                d += base[g.ty[m as usize] as usize];
                m = p;
            }
            for &c in cs {
                if c != usize::MAX {
                    conn_delay[c] = d;
                }
            }
        }
    }
    let rep = sta.analyse(&conn_delay);
    if rep.worst > 0.0 {
        eprintln!("timing: longest path {:.3} ns ({}), {:.0} MHz by the delay model", rep.worst, rep.at, 1000.0 / rep.worst);
    }

    // The design file (README (q), docs/toolchain.md).
    let mut text = format!("# a5pnr design: {top} from {json}\n");
    for l in &pin_lines {
        writeln!(text, "{l}")?;
    }
    // Only edges with bits: pass-through and hardwired hops (class PASS)
    // write nothing, and a decode of the image can't see them. An inverted
    // clear's last routing-tile hop is the inverter's setting instead.
    let inverted: std::collections::HashSet<u32> = lines
        .iter()
        .filter(|(_, ll)| matches!(ll.aclr, Some((_, true))))
        .filter_map(|(&l, _)| g.node(&format!("RPI:X{}Y{}Z0I{}", labs[l].x, labs[l].y, emit::ACLR_RPI)))
        .collect();
    let mut edges: Vec<(String, String)> = routed
        .trees
        .iter()
        .flatten()
        .filter(|&&(a, b)| g.fanout(a).any(|e| g.dst[e] == b && g.class[e] != graph::PASS))
        .filter(|&&(_, b)| !inverted.contains(&b))
        .map(|&(a, b)| (g.name(a), g.name(b)))
        .collect();
    edges.sort();
    edges.dedup();
    for (a, b) in &edges {
        writeln!(text, "edge\t{a}\t{b}")?;
    }
    let (attrs, directs) = emit::blocks(&nl, &packed, &labs, &pl, &lines);
    for (u, a, v) in &attrs {
        writeln!(text, "attr\t{u}\t{a}\t{v}")?;
    }
    for (u, a, v) in &directs {
        writeln!(text, "direct\t{u}\t{a}\t{v}")?;
    }
    std::fs::write(out, text)?;
    std::fs::write(format!("{out}.ffmap"), emit::ffmap(&nl, &packed, &labs, &pl)?)?;
    let labs_used: std::collections::BTreeSet<usize> = pl.at.iter().map(|a| a.0).collect();
    eprintln!(
        "a5pnr: {} ALMs in {} LABs, {} M20Ks, {} nets, {} edges, {} routing iterations, model Fmax {:.0} MHz; place {place_s:.1} s, route {route_s:.1} s, {:.1} s total",
        packed.alms.len(),
        labs_used.len(),
        packed.m20ks.len(),
        rnets.len(),
        edges.len(),
        routed.iterations,
        if rep.worst > 0.0 { 1000.0 / rep.worst } else { 0.0 },
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}
