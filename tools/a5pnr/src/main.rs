//! a5pnr: a small placer and router for the Agilex 5 A5EB013B (README (w)).
//!
//!   a5pnr <db> <graph> <delays.tsv> <netlist.json> <top> <pins.txt> <out design.txt> [seed]
//!
//! Reads a yosys netlist (tools/a5pnr/synth.sh), packs it into ALMs, places
//! them by simulated annealing, routes every net with PathFinder over the
//! routing graph (`<graph>`: tools/rgdump output plus `a5db usable`'s
//! usable.bin) and writes an a5db design file; `a5db build` and `a5db rbf`
//! turn that into a bitstream. pins.txt: `<port> <ball> <3.3|1.8|1.1>` lines.
//! delays.tsv: tools/timing/fit.py's model (node costs).

mod graph;
mod netlist;
mod place;
mod readback;
mod route;
mod timing;

use std::collections::HashMap;
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};

use netlist::{Cell, Sig};
use place::{Alm, End};

const M20K_SDP: [(&str, &str); 20] = [
    ("m20k_nbl_delay", "NBL_DLY1"), ("m20k_float_rdbl_delay", "FLOAT_RDBL_DLY0"), ("m20k_float_rdbl_en", "FLOAT_RDBL_OFF"),
    ("m20k_hs_lp_en", "HIGH_SPEED_EN"), ("m20k_keeper_option", "NKEEPER_EN"), ("m20k_core_pwrmx_ctl", "VCCHG_SEL_OFF"),
    ("m20k_pwrdown", "PWRUP"), ("m20k_rclk0_delay", "RCLK0_DLY2"), ("m20k_rclkctl_delay", "RCLKCTL_DLY3"),
    ("m20k_rclkd_delay", "RCLKD_DLY3"), ("m20k_rclklat_rst", "RCLKLAT_RST_ON"), ("m20k_b_outreg_byp", "B_OUT_REG_OFF"),
    ("m20k_slow_wclk0_delay", "SLOW_WCLK0_DLY2"), ("m20k_tdm_delay", "TDM_DLY1"), ("m20k_wclk0_delay", "WCLK0_DLY3"),
    ("m20k_wclkctl_delay", "WCLKCTL_DLY0"), ("m20k_tdm_phase0_delay", "TDM_PHASE0_DLY2"), ("m20k_wclkd_delay", "WCLKD_DLY2"),
    ("m20k_wclklat_rst", "WCLKLAT_RST_ON"), ("m20k_a_outreg_byp", "A_OUT_REG_OFF"),
];

fn alm_path(k: u8) -> String {
    if k < 5 {
        format!("xtophalf/xale{k}")
    } else {
        format!("xbothalf/xale{}", k - 5)
    }
}

/// asm2's DIRECT lut_mask (MSB first) for a 6-LUT whose WYSIWYG mask is w
/// (bit i: index i = {F, E, D, C, B, A}): stored[j] = ¬w[j ^ 31] (README (h)).
fn lut_stored(w: u64) -> String {
    (0..64).rev().map(|j| if w >> (j ^ 31) & 1 == 1 { '0' } else { '1' }).collect()
}

/// An arithmetic ALM's stored mask: half h's p at bits 32h..32h+15 and g at
/// 32h+16..32h+31, as the WYSIWYG masks are (README (w)).
fn arith_stored(pg: [(u16, u16); 2]) -> String {
    let mut v = 0u64;
    for (h, (p, g)) in pg.iter().enumerate() {
        v |= (*p as u64) << (32 * h) | (*g as u64) << (32 * h + 16);
    }
    (0..64).rev().map(|j| if v >> j & 1 == 1 { '1' } else { '0' }).collect()
}

/// A LUT over k inputs as a 64-entry WYSIWYG mask over A..F (unused inputs
/// don't matter).
fn widen(mask: u64, k: usize) -> u64 {
    (0..64).fold(0u64, |a, i| a | (mask >> (i & ((1 << k) - 1)) & 1) << i)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if let [_, cmd, gdir, nodes @ ..] = args.as_slice() {
        if cmd == "readback" {
            // a5pnr readback <graph> <db> <extract.txt> <pins.txt> <top> <out.v>
            let [db, extract, pins, top, out] = nodes else { bail!("usage: a5pnr readback <graph> <db> <extract> <pins> <top> <out.v>") };
            let g = graph::Graph::load(std::path::Path::new(gdir))?;
            return readback::run(&g, db, extract, pins, top, out);
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
        bail!("usage: a5pnr <db> <graph> <delays.tsv> <netlist.json> <top> <pins.txt> <out design.txt> [seed]");
    };
    let seed: u64 = rest.first().map_or(Ok(1), |s| s.parse())?;
    let t0 = std::time::Instant::now();
    let nl = netlist::read(json, top)?;
    let packed = place::pack(&nl)?;
    let n_arith = packed.alms.iter().filter(|a| matches!(a, Alm::Arith { .. })).count();
    eprintln!("pack: {} cells into {} ALMs ({} arithmetic, {} chains)", nl.cells.len(), packed.alms.len(), n_arith, packed.chains.len());

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
        let is_clock = p.input && matches!(p.net, Sig::Net(n) if Some(n) == packed.clock);
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
    let pl = place::place(&packed, &labs, &place::m20k_sites(db)?, &port_xy, seed)?;
    let lab_uid = |a: usize| {
        let l = labs[pl.at[a].0];
        format!("cw_la_lab_r{}_c{}", l.y, l.x)
    };
    let lab_re = |a: usize, pin: u32| -> Result<u32> {
        let l = labs[pl.at[a].0];
        let name = format!("LAB_RE:X{}Y{}Z0I{pin}", l.x, l.y);
        g.node(&name).with_context(|| format!("no node {name}"))
    };
    // Nets as source and sink nodes.
    let mut rnets = Vec::new();
    for (net, (d, sinks)) in &packed.nets {
        let node = |e: &End| -> Result<u32> {
            match *e {
                End::Out { alm, h, ff } => lab_re(alm, 89 + 4 * pl.at[alm].1 as u32 + 2 * h as u32 + (!ff) as u32),
                End::In { alm, pin } => lab_re(alm, 8 * pl.at[alm].1 as u32 + pin as u32),
                End::Clock { alm } => lab_re(alm, 83),
                End::Port(p) => port_node.get(&p).copied().with_context(|| format!("port {} has no node", nl.ports[p].name)),
                End::M20k { m, pin } => {
                    let (x, y) = pl.m20k_at[m];
                    let name = format!("MEDIUM_EAB_RE:X{x}Y{y}Z0I{pin}");
                    g.node(&name).with_context(|| format!("no node {name}"))
                }
            }
        };
        let ends: Vec<(End, u32)> = sinks.iter().map(|e| Ok((*e, node(e)?))).collect::<Result<_>>()?;
        let mut ss: Vec<u32> = ends.iter().map(|e| e.1).collect();
        ss.sort();
        ss.dedup();
        rnets.push((route::Net { name: nl.nets[*net as usize].clone(), source: node(d)?, sinks: ss, clock: Some(*net) == packed.clock }, *net, ends));
    }
    rnets.sort_by_key(|n| (!n.0.clock, std::cmp::Reverse(n.0.sinks.len())));
    let net_ends: Vec<(u32, Vec<(End, u32)>)> = rnets.iter().map(|n| (n.1, n.2.clone())).collect();
    let rnets: Vec<route::Net> = rnets.into_iter().map(|n| n.0).collect();
    let mut delay: HashMap<String, f32> = HashMap::new();
    for l in std::fs::read_to_string(delays).context("reading delays")?.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() >= 3 {
            delay.insert(f[0].to_owned(), f[2].parse()?);
        }
    }
    let t1 = std::time::Instant::now();
    let routed = route::route(&g, &rnets, &route::base_costs(&g, &delay), 60)?;
    let hops: usize = routed.trees.iter().map(|t| t.len()).sum();
    eprintln!("route: {} nets, {} hops, {} iterations, {:.1} s", rnets.len(), hops, routed.iterations, t1.elapsed().as_secs_f64());

    // Delay estimate (README (u)): each routed hop costs its node type's
    // mean, cells their arcs' means.
    let model = timing::Model { d: delay.clone() };
    let mut routed_delay: HashMap<(u32, End), f32> = HashMap::new();
    for (ni, tree) in routed.trees.iter().enumerate() {
        let parent: HashMap<u32, u32> = tree.iter().map(|&(a, b)| (b, a)).collect();
        let (net, ends) = &net_ends[ni];
        for (e, n) in ends {
            let mut d = 0.0f32;
            let mut m = *n;
            while let Some(&p) = parent.get(&m) {
                d += delay.get(&format!("re:{}", g.type_name(m))).copied().unwrap_or(0.05);
                m = p;
            }
            routed_delay.insert((*net, *e), d);
        }
    }
    let mut tm = timing::Timing::new(&nl, &packed, &model, &routed_delay);
    let (worst, at) = tm.worst();
    if worst > 0.0 {
        eprintln!("timing: longest path to a flop {worst:.3} ns ({at}), {:.0} MHz by the delay model", 1000.0 / worst);
    }

    // The design file (README (q), docs/toolchain.md).
    let mut text = format!("# a5pnr design: {top} from {json}\n");
    for l in &pin_lines {
        writeln!(text, "{l}")?;
    }
    // Only edges with bits: pass-through and hardwired hops (class PASS)
    // write nothing, and a decode of the image can't see them.
    let mut edges: Vec<(String, String)> = routed
        .trees
        .iter()
        .flatten()
        .filter(|&&(a, b)| g.fanout(a).any(|e| g.dst[e] == b && g.class[e] != graph::PASS))
        .map(|&(a, b)| (g.name(a), g.name(b)))
        .collect();
    edges.sort();
    edges.dedup();
    for (a, b) in &edges {
        writeln!(text, "edge\t{a}\t{b}")?;
    }
    let mut attrs: Vec<(String, String, String)> = Vec::new();
    let mut directs: Vec<(String, String, String)> = Vec::new();
    let mut ff_labs = std::collections::BTreeSet::new();
    for (a, alm) in packed.alms.iter().enumerate() {
        let (uid, k) = (lab_uid(a), pl.at[a].1);
        let p = alm_path(k);
        let mut set = |path: String, v: &str| attrs.push((uid.clone(), path, v.to_owned()));
        // Register settings as Quartus writes them for every used ALM
        // (designs/blinky_a5db, ho): LAB clock 0, clear line 2, the LAB's
        // enable and clear selects; the unused lines idle inactive.
        let regs = |set: &mut dyn FnMut(String, &str), h: u8, ff: bool, comb: bool, input: &str| {
            let r = format!("{p}/xcw_la_le_reg{h}");
            // ARITH_SUMOUT is reg_input's idle setting (both bits 0): left out.
            if ff {
                set(format!("{r}/xlabclk_mux/regclk"), "DFF_LABCLK0");
                if input != "ARITH_SUMOUT" {
                    set(format!("{r}/xreg0/reg_input"), input);
                }
                set(format!("{r}/xreg0/register_comb"), "DFF_LATCH");
                set(format!("{r}/xreg0/leout"), "LEOUT");
            }
            if comb {
                if input != "ARITH_SUMOUT" {
                    set(format!("{r}/xreg1/reg_input"), input);
                }
                set(format!("{r}/xreg1/register_comb"), "WIRE_MODE");
                set(format!("{r}/xreg1/leout"), "LEOUT");
            }
        };
        match alm {
            Alm::Lut { lut, ff, comb } => {
                let w = match lut {
                    Some(l) => {
                        let Cell::Lut { ins, mask, .. } = &nl.cells[*l] else { unreachable!() };
                        widen(*mask, ins.len())
                    }
                    None => widen(0b10, 1),
                };
                directs.push((uid.clone(), format!("{p}/xcw_la_le_lut/lut_mask"), lut_stored(w)));
                regs(&mut set, 0, ff.is_some(), *comb, "LUTOUT_6LUT");
                if ff.is_some() {
                    ff_labs.insert(uid.clone());
                }
            }
            Alm::Arith { halves } => {
                let mut pg = [(0u16, 0u16); 2];
                for (h, half) in halves.iter().enumerate() {
                    let Some(half) = half else { continue };
                    let Cell::Arith { p: pm, g: gm, .. } = nl.cells[half.cell] else { unreachable!() };
                    pg[h] = (pm, gm);
                    regs(&mut set, h as u8, half.ff.is_some(), half.comb, "ARITH_SUMOUT");
                    if half.ff.is_some() {
                        ff_labs.insert(uid.clone());
                    }
                }
                set(format!("{p}/xcw_la_le_lut/adder_mode"), "ENABLE_ADDER");
                set(format!("{p}/xce_la_le_adder/adder_or_packed_mode"), "ENABLE_ADDER_OR_PACKED");
                directs.push((uid.clone(), format!("{p}/xcw_la_le_lut/lut_mask"), arith_stored(pg)));
            }
        }
        set(format!("{p}/xcw_la_le_reg0/xreg_aclr/regclr"), "CLR2");
        set(format!("{p}/xcw_la_le_reg1/xreg_aclr/regclr"), "CLR2");
        set(format!("{p}/xcw_la_le_regctrl/xcw_la_le_regctrl_reg/ce_sel"), "CE");
        set(format!("{p}/xcw_la_le_regctrl/xcw_la_le_regctrl_reg/clr_sel"), "CLR");
    }
    for u in &ff_labs {
        attrs.push((u.clone(), "xsecmux/xcw_la_lcb_clkgen/xlabclk0/smxclk_tie_off".into(), "DISABLE_TIEOFF".into()));
    }
    // The LAB a chain starts in: Quartus sets these two in every such LAB
    // (ar, ho); what they select is [inf] (README (w)).
    for c in &packed.chains {
        let u = lab_uid(c[0]);
        attrs.push((u.clone(), "xsecmux/xcw_la_lcb_adder/secmux_bto".into(), "DISABLE_BTO".into()));
        attrs.push((u, "xsecmux/xcw_la_lcb_adder/hipi_clken".into(), "COMB_PATH".into()));
    }
    // An M20K as a 512 x 32 simple dual-port RAM: these 20 settings are all
    // mm's such block differs from the idle state in; widths and modes are
    // the idle ones (README (w)).
    for &(x, y) in &pl.m20k_at {
        for (a, v) in M20K_SDP {
            attrs.push((format!("cw_mm_m20k_r{y}_c{x}"), format!("xdigicore/{a}"), v.to_string()));
        }
    }
    attrs.sort();
    attrs.dedup();
    for (u, a, v) in &attrs {
        writeln!(text, "attr\t{u}\t{a}\t{v}")?;
    }
    for (u, a, v) in &directs {
        writeln!(text, "direct\t{u}\t{a}\t{v}")?;
    }
    std::fs::write(out, text)?;
    // Which flop holds which netlist register (for simulation only: the
    // readback check lines up RTL state with the image's flops by it).
    let mut map = String::new();
    for (a, alm) in packed.alms.iter().enumerate() {
        let l = labs[pl.at[a].0];
        let k = pl.at[a].1 as u32;
        let ffs: Vec<(u32, usize)> = match alm {
            Alm::Lut { ff, .. } => ff.iter().map(|f| (0, *f)).collect(),
            Alm::Arith { halves } => halves.iter().enumerate().filter_map(|(h, x)| x.as_ref().and_then(|x| x.ff).map(|f| (h as u32, f))).collect(),
        };
        for (h, f) in ffs {
            let Cell::Ff { q, .. } = nl.cells[f] else { unreachable!() };
            writeln!(map, "{}\tl{}_{}_o{}", nl.nets[q as usize], l.x, l.y, 89 + 4 * k + 2 * h)?;
        }
    }
    std::fs::write(format!("{out}.ffmap"), map)?;
    let labs_used: std::collections::BTreeSet<usize> = pl.at.iter().map(|a| a.0).collect();
    eprintln!(
        "a5pnr: {} ALMs in {} LABs, {} nets, {} edges, {} routing iterations, {:.1} s total",
        packed.alms.len(),
        labs_used.len(),
        rnets.len(),
        edges.len(),
        routed.iterations,
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}
