//! The netlist an image holds, rebuilt from `a5db extract`'s decode of it
//! and written as Verilog, for simulation against the source design
//! (README (w), (y)). Nothing comes from a5pnr's own intent: connectivity is
//! the image's decoded mux edges plus the graph's pass-through edges, cells
//! are the decoded LAB, routing-tile and M20K attributes, LUT masks and M20K
//! contents, ports are the design file's pins at their boundary nodes
//! (README (t)). What the image does that this model doesn't know is an
//! error, not a guess.

use std::collections::{BTreeMap, VecDeque};
use crate::det::{HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};

use crate::graph::{Graph, IDLE, NO, PASS};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum Src {
    /// LAB (x, y) output pin.
    Lab(u32, u32, u32),
    /// Top-level input port k.
    Port(usize),
    /// M20K (x, y) output pin.
    M20k(u32, u32, u32),
    /// The input register of input port k (README (z10)).
    IoIn(usize),
    /// DSP block (x, y) output pin (README (z11)).
    Dsp(u32, u32, u32),
}

/// The M20K settings a5pnr's 512 x 32 use writes (emit::M20K_SDP); any
/// other differing M20K attribute is outside the model.
const M20K_KNOWN: [&str; 2] = ["xdigicore/m20k_a_outreg_byp", "xdigicore/m20k_b_outreg_byp"];

pub fn run(g: &Graph, db: &str, extract: &str, pins: &str, top: &str, out: &str) -> Result<()> {
    let text = std::fs::read_to_string(extract).with_context(|| format!("reading {extract}"))?;
    let mut active: HashSet<(u32, u32)> = HashSet::default();
    let mut attr: HashMap<(String, String), String> = HashMap::default();
    let mut balls: Vec<(String, String)> = Vec::new();
    let mut plls: std::collections::BTreeSet<String> = Default::default();
    for l in text.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        match f[0] {
            "pin" => {
                let (b, u) = f[1].split_once('=').context("pin")?;
                balls.push((b.to_owned(), u.split(':').next().unwrap().to_owned()));
            }
            "edge" => {
                active.insert((g.node(f[1]).with_context(|| f[1].to_owned())?, g.node(f[2]).with_context(|| f[2].to_owned())?));
            }
            // An IOPLL's settings (tools/pll/iopll.py): its bank's PLL is on.
            "attr" | "direct" | "bit" if f[1].starts_with("hvio") && f[2].starts_with("xpllwrap_top/") => {
                plls.insert(f[1].to_owned());
            }
            // A clock pin's entry into the clock network (`a5db pins`, README
            // (r)): the edges past it are what the model follows. Quartus'
            // pl sets V16's 2:1 ioclk mux with V16 only a PLL reference; pa,
            // with V16 a PLL reference too, leaves it idle [obs].
            "attr" if f[1].starts_with("hvio") && f[2].starts_with("xif/xif_clk_mux/") => {}
            "attr" | "direct" => {
                attr.insert((f[1].to_owned(), f[2].to_owned()), f[3].to_owned());
            }
            "bit" | "raw" => bail!("{extract}: the image has bits outside the design vocabulary: {l}"),
            _ => {}
        }
    }
    // Routing-tile attributes: only the clear inverters are modelled. An
    // inverted clear's last hop (RPII → RPI) conducts, inverted.
    let mut inverted_lab: HashSet<(u32, u32)> = HashSet::default();
    // I/O registers (README (z10)): the routing tiles' programmable
    // inverters in front of the I/O blocks, and the blocks' register
    // settings, per pin.
    let rpis = crate::ioreg::rpis(g, db)?;
    let io_pins = crate::ioreg::pins(g, db)?;
    let mut inverted_rpi: HashSet<u32> = HashSet::default();
    let mut io_attr: HashMap<(String, String), String> = HashMap::default();
    for ((u, a), v) in &attr {
        if u.starts_with("cw_rt_hiol") || u.starts_with("cw_rt_vio") {
            let Some(path) = a.strip_suffix("/rpi_sel") else { bail!("{u} {a} = {v}: routing attribute outside the model") };
            let n = *rpis.get(&(u.clone(), path.to_owned())).with_context(|| format!("{u} {path}: no such inverter in io/rpi.tsv"))?;
            if v != "INV" {
                bail!("{u} {a} = {v}: outside the model");
            }
            inverted_rpi.insert(n);
            continue;
        }
        if (u.starts_with("hvio") || u.starts_with("pio")) && (a.contains("xiogpio_reg/") || a.contains("/x_gpio_") || a.contains("/xflyby/")) {
            io_attr.insert((u.clone(), a.clone()), v.clone());
            continue;
        }
        let Some(rest) = u.strip_prefix("cw_rt_lab_base_r") else {
            if u.starts_with("cw_rt_") || u.starts_with("hvio") || u.starts_with("pio") {
                bail!("{u} {a} = {v}: routing or I/O attribute outside the model");
            }
            continue;
        };
        let (y, x) = rest.split_once("_c").context("uid")?;
        let (x, y): (u32, u32) = (x.parse()?, y.parse()?);
        match (a.as_str(), v.as_str()) {
            ("iss/xclrb_rpi/sel", "INV") => {
                let (Some(a), Some(b)) = (g.node(&format!("RPII:X{x}Y{y}Z0I5")), g.node(&format!("RPI:X{x}Y{y}Z0I5"))) else { bail!("no RPI at X{x}Y{y}") };
                active.insert((a, b));
                inverted_lab.insert((x, y));
            }
            _ => bail!("{u} {a} = {v}: routing-tile attribute outside the model"),
        }
    }
    // Port names by ball (pins.txt) and boundary nodes (io/pin_nodes.tsv).
    let mut port_of: HashMap<String, String> = HashMap::default();
    for l in std::fs::read_to_string(pins)?.lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() >= 2 && !f[0].starts_with('#') {
            port_of.insert(f[1].to_owned(), f[0].to_owned());
        }
    }
    let mut node_of: HashMap<(String, String), u32> = HashMap::default();
    let mut io96: HashMap<String, bool> = HashMap::default();
    for l in std::fs::read_to_string(format!("{db}/io/pin_nodes.tsv"))?.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 5 && !l.starts_with('#') && f[0] != "ball" {
            if let Some(n) = g.node(f[3]) {
                node_of.insert((f[0].to_owned(), f[2].to_owned()), n);
            }
            io96.insert(f[0].to_owned(), f[1].starts_with("pio"));
        }
    }
    let mut inputs: Vec<(String, u32)> = Vec::new();
    let mut outputs: Vec<(String, u32)> = Vec::new();
    // Which pins have which registers (the 4:1 muxes in front of the
    // buffer and behind the input select the registered path, SEL1).
    let io_has = |ball: &str, sub: &str| -> bool {
        io_pins.get(ball).is_some_and(|p| io_attr.get(&(p.uid.clone(), format!("{}/{sub}/a_rb_mux_sel", p.path))).is_some_and(|v| v == "SEL1"))
    };
    for (b, u) in &balls {
        let name = port_of.get(b).with_context(|| format!("ball {b} isn't in {pins}"))?.clone();
        let role = match u.as_str() {
            "clock_in" => "clock",
            "out" => "out",
            "in0" | "in1" => u.as_str(),
            // A registered input leaves on in1, IO96 too [obs, iq].
            "in" | "in_reg" if io_has(b, "xio_gpio_ireg_4to1_mux1") => "in1",
            "in" => if io96[b] { "in0" } else { "in1" },
            _ => bail!("pin use {u} is outside the model"),
        };
        let n = *node_of.get(&(b.clone(), role.to_owned())).with_context(|| format!("{b} {role}"))?;
        if role == "out" { outputs.push((name, n)) } else { inputs.push((name, n)) }
    }
    // Used LABs and M20Ks.
    // A LAB or an MLAB used as logic (Quartus does, fr): the same ALMs on
    // the same LAB_RE pins, `xcw_ml_le_*` for `xcw_la_le_*` [obs].
    let mut labs: BTreeMap<(u32, u32), &str> = BTreeMap::new();
    let mut m20ks: BTreeMap<(u32, u32), ()> = BTreeMap::new();
    let mut dsps: BTreeMap<(u32, u32), ()> = BTreeMap::new();
    for (u, _) in attr.keys() {
        if let Some(rest) = u.strip_prefix("cw_mm_m20k_r") {
            let (y, x) = rest.split_once("_c").context("uid")?;
            m20ks.insert((x.parse()?, y.parse()?), ());
        } else if let Some(rest) = u.strip_prefix("cw_la_lab_r") {
            let (y, x) = rest.split_once("_c").context("uid")?;
            labs.insert((x.parse()?, y.parse()?), "la");
        } else if let Some(rest) = u.strip_prefix("cw_ml_mlab_r") {
            let (y, x) = rest.split_once("_c").context("uid")?;
            labs.insert((x.parse()?, y.parse()?), "ml");
        } else if let Some(rest) = u.strip_prefix("cw_dssm_r") {
            let (y, x) = rest.split_once("_c").context("uid")?;
            dsps.insert((x.parse()?, y.parse()?), ());
        } else if !(u.starts_with("cw_rt_lab_base") || u.starts_with("cw_rt_hiol") || u.starts_with("cw_rt_vio") || u.starts_with("hvio") || u.starts_with("pio")) {
            bail!("{u}: block type outside the model");
        }
    }
    // Every source's reach over conducting edges: decoded mux edges and the
    // graph's pass-through/hardwired ones.
    let mut sources: Vec<(Src, u32)> = Vec::new();
    for &(x, y) in labs.keys() {
        // Register slots (89..128) and the ALMs' direct LUT outputs
        // (129..148, README (z3)).
        for pin in 89..=148 {
            if let Some(n) = g.node(&format!("LAB_RE:X{x}Y{y}Z0I{pin}")) {
                sources.push((Src::Lab(x, y, pin), n));
            }
        }
    }
    let ball_of: HashMap<String, String> = port_of.iter().map(|(b, p)| (p.clone(), b.clone())).collect();
    for (k, (name, n)) in inputs.iter().enumerate() {
        let registered = ball_of.get(name).is_some_and(|b| io_has(b, "xio_gpio_ireg_4to1_mux1"));
        sources.push((if registered { Src::IoIn(k) } else { Src::Port(k) }, *n));
    }
    // A PLL passes its reference clock on (for this check its frequency
    // doesn't matter): V16's PLL output 0 is HVIOWR_RE I16 (README (y)).
    for u in &plls {
        let (ball, node) = match u.as_str() {
            "hvio_3_1" => ("V16", "HVIOWR_RE:X121Y30Z500I16"),
            _ => bail!("PLL in {u}: outside the model"),
        };
        let k = inputs.iter().position(|(n, _)| port_of.get(ball) == Some(n)).with_context(|| format!("PLL reference {ball} isn't an input"))?;
        sources.push((Src::Port(k), g.node(node).context("PLL output node")?));
    }
    for &(x, y) in m20ks.keys() {
        for pin in 83..123u32 {
            if let Some(n) = g.node(&format!("MEDIUM_EAB_RE:X{x}Y{y}Z0I{pin}")) {
                sources.push((Src::M20k(x, y, pin), n));
            }
        }
    }
    for &(x, y) in dsps.keys() {
        for k in 0..54 {
            let pin = crate::dsp::p_pin(k) as u32;
            if let Some(n) = g.node(&format!("MP_MAC_RE:X{x}Y{y}Z0I{pin}")) {
                sources.push((Src::Dsp(x, y, pin), n));
            }
        }
    }
    let single = g.single_fanin();
    // A clock-network mux no decoded edge enters is at its idle setting and
    // passes the input that setting selects: Quartus' oc takes CLOCK2_50
    // through Vertical_seam_tap_mux I161 → I125 that way [obs, README (z6)].
    // In the fabric (an unrouted LAB pin) that is [unk], so not there.
    let driven: HashSet<u32> = active.iter().map(|&(_, d)| d).collect();
    let clock_net = |d: u32| matches!(g.type_name(d), "Switchbox_clock_mux" | "Vertical_seam_tap_mux" | "SPINE_TAP_RE" | "SCLKMUX_RE" | "DCM_mux" | "HSSI_P2C_RE" | "ROWCLK_TAPOFF" | "GLOBAL_LOCAL_SEL") || g.type_name(d).starts_with("CE_CC_") || g.type_name(d).starts_with("CW_CC_");
    let follow = |e: usize, m: u32, d: u32| {
        g.class[e] == PASS || (g.class[e] == NO && single[d as usize]) || active.contains(&(m, d)) || (g.class[e] == IDLE && clock_net(d) && !driven.contains(&d))
    };
    let mut reach: HashMap<u32, Src> = HashMap::default();
    let mut conflict: Vec<u32> = Vec::new();
    for &(s, n0) in &sources {
        let mut q = VecDeque::from([n0]);
        let mut seen = HashSet::from_iter([n0]);
        while let Some(m) = q.pop_front() {
            for e in g.fanout(m) {
                let d = g.dst[e];
                if follow(e, m, d) && seen.insert(d) {
                    if let Some(o) = reach.insert(d, s) {
                        if o != s {
                            conflict.push(d);
                        }
                    }
                    q.push_back(d);
                }
            }
        }
    }
    // A node two sources reach over pass-through edges (some clock-network
    // nodes have several, README (y)) is unknown, and so is everything it
    // reaches; that only matters if a used pin is among them.
    let mut unknown: HashSet<u32> = HashSet::default();
    let mut q: VecDeque<u32> = conflict.iter().copied().collect();
    while let Some(m) = q.pop_front() {
        if !unknown.insert(m) {
            continue;
        }
        for e in g.fanout(m) {
            let d = g.dst[e];
            if follow(e, m, d) {
                q.push_back(d);
            }
        }
    }
    let used_unknown = std::cell::RefCell::new(Vec::new());
    let sig = |n: Option<u32>| -> Option<String> {
        if let Some(n) = n {
            if unknown.contains(&n) {
                used_unknown.borrow_mut().push(g.name(n));
                return Some("1'bx".into());
            }
        }
        n.and_then(|n| reach.get(&n)).map(|s| match s {
            Src::Lab(x, y, p) => format!("l{x}_{y}_o{p}"),
            Src::Port(k) => inputs[*k].0.replace(['[', ']'], "_"),
            Src::M20k(x, y, p) => format!("m{x}_{y}_q{p}"),
            Src::IoIn(k) => format!("io_{}_i", inputs[*k].0.replace(['[', ']'], "_")),
            Src::Dsp(x, y, p) => format!("d{x}_{y}_q{p}"),
        })
    };
    let lab_node = |x: u32, y: u32, pin: u32| g.node(&format!("LAB_RE:X{x}Y{y}Z0I{pin}"));
    let lab_sig = |x: u32, y: u32, pin: u32| sig(lab_node(x, y, pin)).unwrap_or_else(|| "1'bx".into());
    let mut head = String::new();
    let mut v = String::new();
    writeln!(head, "// Rebuilt by a5pnr readback from {extract} (README (w), (y)).")?;
    let ports: Vec<String> = inputs.iter().map(|p| format!("input wire {}", p.0.replace(['[', ']'], "_"))).chain(outputs.iter().map(|p| format!("output wire {}", p.0.replace(['[', ']'], "_")))).collect();
    writeln!(head, "module {top}_readback ({});", ports.join(", "))?;
    // LUTs as mux trees: an unknown select bit gives a known output when
    // both halves agree, as in silicon (a vector index with an x is all x).
    writeln!(head, "  function automatic lut4(input [15:0] m, input [3:0] i); reg [7:0] a; reg [3:0] b; reg [1:0] c;")?;
    writeln!(head, "    begin a = i[3] ? m[15:8] : m[7:0]; b = i[2] ? a[7:4] : a[3:0]; c = i[1] ? b[3:2] : b[1:0]; lut4 = i[0] ? c[1] : c[0]; end")?;
    writeln!(head, "  endfunction")?;
    writeln!(head, "  function automatic lut5(input [31:0] m, input [4:0] i);")?;
    writeln!(head, "    begin lut5 = i[4] ? lut4(m[31:16], i[3:0]) : lut4(m[15:0], i[3:0]); end")?;
    writeln!(head, "  endfunction")?;
    let alm = |k: u32| if k < 5 { format!("xtophalf/xale{k}") } else { format!("xbothalf/xale{}", k - 5) };
    let lab_uid = |x: u32, y: u32| match labs.get(&(x, y)) {
        Some(&"ml") => format!("cw_ml_mlab_r{y}_c{x}"),
        _ => format!("cw_la_lab_r{y}_c{x}"),
    };
    let lab_attr = |x: u32, y: u32, a: &str| attr.get(&(lab_uid(x, y), a.to_owned())).map(String::as_str);
    let kind = |x: u32, y: u32| labs.get(&(x, y)).copied().unwrap_or("la");
    let is_arith = |x: u32, y: u32, k: u32| lab_attr(x, y, &format!("{}/xcw_{}_le_lut/adder_mode", alm(k), kind(x, y))) == Some("ENABLE_ADDER");
    // Declarations first: cells refer to each other in any order.
    let mut d = String::new();
    let mut nffs = 0;
    let mut nalms = 0;
    for (&(x, y), &kd) in &labs {
        let uid = lab_uid(x, y);
        // An MLAB's LUT and register-control blocks: xcw_ml_le_*.
        let lut = format!("xcw_{kd}_le_lut");
        let regctrl = format!("xcw_{kd}_le_regctrl");
        // The LAB's control lines (README (y)): clock 0 (83), enables
        // CLKEN0/1 (80, 81), synchronous clear (82), clears CLR1 (86,
        // through the routing tile's clrb inverter) and CLR2 (85). An
        // unrouted clear is inactive (as in Quartus' images).
        let clk = lab_sig(x, y, 83);
        // LAB clock 1: `regclk = DFF_LABCLK1` (Quartus' MLAB RAM output
        // registers, ml) [obs].
        let clk1 = lab_sig(x, y, 84);
        let ces = [lab_sig(x, y, 80), lab_sig(x, y, 81)];
        // An MLAB in RAM mode [obs, ml: README (z5)]: Quartus marks every MLAB
        // it uses as logic with `lutram_mode_vcchg_power_saving` (fr, cq and
        // cql: 207 of 207) and leaves it idle in the RAM's. Each half of an
        // ALM is one bit of a 32-word RAM held in its 32 mask bits: read at
        // {E, D1, C1, B, A} through LUTOUT_5LUT as in logic mode, written on
        // LAB clock 0 when CLKEN0 (80) is high, at the address on LAB_RE 156,
        // 157, 160, 161, 155 (bits 0..4), with half 0's data from C0 and half
        // 1's from D0. An address pin left unrouted (a 16-word RAM's bit 4)
        // reads 1 on both ports [obs: Quartus' mi stores a 16-word RAM's
        // initial word a at 32h + 16 + a, README (z6)].
        let ram = kd == "ml" && lab_attr(x, y, "xpwrmgmt/lutram_mode_vcchg_power_saving") != Some("ENABLE_POWER_SAVING");
        let pin1 = |pin: u32| sig(lab_node(x, y, pin)).unwrap_or_else(|| "1'b1".into());
        let waddr = format!("{{{}, {}, {}, {}, {}}}", pin1(155), pin1(161), pin1(160), pin1(157), pin1(156));
        let sclr = lab_sig(x, y, 82);
        let clr1 = match sig(lab_node(x, y, 86)) {
            Some(s) if inverted_lab.contains(&(x, y)) => format!("~{s}"),
            Some(s) => s,
            None => "1'b0".into(),
        };
        let clr2 = sig(lab_node(x, y, 85)).unwrap_or_else(|| "1'b0".into());
        writeln!(d, "  wire l{x}_{y}_clr1 = {clr1}, l{x}_{y}_clr2 = {clr2};")?;
        for (a, val) in [("xsecmux/xcw_la_lcb_clkgen/xlabclk0/smxclk_tie_off", "DISABLE_TIEOFF"), ("xsecmux/xcw_la_lcb_adder/secmux_bto", "DISABLE_BTO"), ("xsecmux/xcw_la_lcb_adder/hipi_clken", "COMB_PATH")] {
            if lab_attr(x, y, a).is_some_and(|v| v != val) {
                bail!("{uid} {a}: setting outside the model");
            }
        }
        for ((u, a), val) in &attr {
            // The LAB clock's programmable delay (Quartus' CPU without
            // retiming, cqr: 219 LABs): timing only, nothing to simulate [inf].
            if *u == uid && (a == "xsecmux/xcw_la_lcb_clkgen/xlabclk0/smxclk_dly" || (a == "xsecmux/xcw_la_lcb_clkgen/xlabclk0/smxclk_dly_off" && val == "ENABLE_DELAY")) {
                continue;
            }
            if *u == uid && a.starts_with("xsecmux") && ![
                "xsecmux/xcw_la_lcb_clkgen/xlabclk0/smxclk_tie_off",
                "xsecmux/xcw_la_lcb_adder/secmux_bto",
                "xsecmux/xcw_la_lcb_adder/hipi_clken",
            ]
            .contains(&a.as_str())
            {
                bail!("{uid} {a} = {val}: LAB setting outside the model");
            }
            // Every ALM setting the model reads or knows to be inert;
            // anything else stops the readback (README (z6): it once passed
            // over unknown LAB settings and only checked MLABs). Settings
            // Quartus writes in MLABs used as logic have no function in a
            // LAB, whose ALMs work with them idle [inf, fr].
            if *u == uid && a.contains('/') && !a.starts_with("xsecmux") {
                let leaf = a.rsplit('/').next().unwrap();
                let ok = match (leaf, val.as_str()) {
                    ("lut_mask" | "register_comb" | "reg_input" | "regclk" | "regclk_ens" | "regclr", _) => true,
                    ("adder_mode", "ENABLE_ADDER") | ("adder_or_packed_mode", "ENABLE_ADDER_OR_PACKED") => true,
                    ("leout", "LEOUT") | ("sclr_sel", "SCLR") | ("lut5_powerdown", "LUT5_OUT") | ("lut6_powerdown", "LUT6_OUT") => true,
                    // xreg0's enable, or the ALM's register-control enables
                    // that go with any enabled or cleared flop.
                    ("ce_sel", "CE") | ("clr_sel", "CLR") => true,
                    ("ram_mode", "LUT_MODE") | ("lutram_mode_vcchg_power_saving", "ENABLE_POWER_SAVING") => kd == "ml",
                    _ => false,
                };
                if !ok {
                    bail!("{uid} {a} = {val}: ALM setting outside the model");
                }
            }
        }
        for k in 0..10u32 {
            let p = alm(k);
            let mask = attr.get(&(uid.clone(), format!("{p}/{lut}/lut_mask")));
            let get = |a: &str| attr.get(&(uid.clone(), format!("{p}/{a}"))).map(String::as_str);
            let used = (0..2).any(|h| (0..2).any(|r| get(&format!("xcw_la_le_reg{h}/xreg{r}/register_comb")).is_some()));
            if mask.is_none() && !used {
                continue;
            }
            nalms += 1;
            let stored: Vec<u8> = mask.map_or(vec![1; 64], |m| m.bytes().rev().map(|c| c - b'0').collect());
            let pin = |q: u32| lab_sig(x, y, 8 * k + q);
            let n = format!("l{x}_{y}_a{k}");
            let arith = get(&format!("{lut}/adder_mode")) == Some("ENABLE_ADDER");
            let mut outs: [String; 2] = [String::new(), String::new()];
            let mut lut6 = String::new();
            let mut five: [String; 2] = [String::new(), String::new()];
            if arith {
                // Half h: p over {D, C, B, A} at 32h, g at 32h + 16 (README (w)).
                let (a, b) = (pin(3), pin(4));
                for h in 0..2u32 {
                    let (c, d_in) = if h == 0 { (pin(2), pin(1)) } else { (pin(5), pin(6)) };
                    let bits = |off: usize| -> String { (0..16).rev().map(|i| (b'0' + stored[off + i]) as char).collect() };
                    // The carry in (README (y)): from the previous half; into
                    // ALM 5 only with DISABLE_BTO, into ALM 0 only from a
                    // LAB above with hipi_clken COMB_PATH; otherwise 0.
                    let cin = if h == 1 {
                        format!("{n}_co0")
                    } else if k == 0 {
                        if lab_attr(x, y + 1, "xsecmux/xcw_la_lcb_adder/hipi_clken") == Some("COMB_PATH") && is_arith(x, y + 1, 9) {
                            format!("l{x}_{}_a9_co1", y + 1)
                        } else {
                            "1'b0".into()
                        }
                    } else if k == 5 {
                        if lab_attr(x, y, "xsecmux/xcw_la_lcb_adder/secmux_bto") == Some("DISABLE_BTO") && is_arith(x, y, 4) {
                            format!("l{x}_{y}_a4_co1")
                        } else {
                            "1'b0".into()
                        }
                    } else if is_arith(x, y, k - 1) {
                        format!("l{x}_{y}_a{}_co1", k - 1)
                    } else {
                        "1'bx".into()
                    };
                    writeln!(d, "  wire [15:0] {n}_p{h} = 16'b{}, {n}_g{h} = 16'b{};", bits(32 * h as usize), bits(32 * h as usize + 16))?;
                    writeln!(d, "  wire [3:0] {n}_i{h}; wire {n}_pp{h}, {n}_co{h}, {n}_s{h};")?;
                    writeln!(v, "  assign {n}_i{h} = {{{dd}, {c}, {b}, {a}}};", dd = d_in)?;
                    writeln!(v, "  assign {n}_pp{h} = lut4({n}_p{h}, {n}_i{h});")?;
                    writeln!(v, "  assign {n}_co{h} = {n}_pp{h} ? {cin} : lut4({n}_g{h}, {n}_i{h});")?;
                    writeln!(v, "  assign {n}_s{h} = {cin} ^ {n}_pp{h};")?;
                    outs[h as usize] = format!("{n}_s{h}");
                    // The half's whole 5-LUT, p below g, over {E_h, D, C, B,
                    // A}: Quartus feeds an unrelated flop through it (p = 0,
                    // g = 1 passes E) in a chain's spare half [obs, mi].
                    let e = if h == 0 { pin(0) } else { pin(7) };
                    let w: String = (0..32usize).rev().map(|i| (b'0' + stored[32 * h as usize + i]) as char).collect();
                    writeln!(d, "  wire {n}_f{h};")?;
                    writeln!(v, "  assign {n}_f{h} = lut5(32'b{w}, {{{e}, {d_in}, {c}, {b}, {a}}});", d_in = d_in)?;
                    five[h as usize] = format!("{n}_f{h}");
                }
            } else if ram {
                let w: String = (0..64usize).rev().map(|i| (b'0' + stored[i]) as char).collect();
                let lp = |q: u32| sig(lab_node(x, y, 8 * k + q)).unwrap_or_else(|| "1'b1".into());
                let raddr = format!("{{{}, {}, {}, {}, {}}}", lp(7), lp(6), lp(5), lp(4), lp(3));
                writeln!(d, "  reg [63:0] {n}_m = 64'b{w}; wire {n}_q0, {n}_q1;")?;
                for h in 0..2u32 {
                    let data = if h == 0 { lp(2) } else { lp(1) };
                    writeln!(v, "  assign {n}_q{h} = {n}_m[{} + {raddr}];", 32 * h)?;
                    writeln!(v, "  always @(posedge {clk}) if ({}) {n}_m[{} + {waddr}] <= {data};", ces[0], 32 * h)?;
                    outs[h as usize] = format!("{n}_q{h}");
                }
            } else {
                // The pins are E0, D0, C0, A, B, C1, D1, E1 [obs, fr's atoms].
                // Half h's 5-LUT (`LUTOUT_5LUT`) is stored bits 32h.. over
                // {E_h, D_h, C_h, B, A}; the 6-LUT (`LUTOUT_6LUT`) is
                // ¬(E1 ? low : high) with E0 as both halves' E [obs, Quartus'
                // fr against its RTL, README (z3)]. That the 6-LUT's high half
                // reads C1, D1 is [inf]: Quartus and a5pnr route C and D to both.
                let w: String = (0..64usize).rev().map(|i| (b'0' + stored[i]) as char).collect();
                writeln!(d, "  wire [63:0] {n}_w = 64'b{w}; wire {n}_lo, {n}_hi6, {n}_hi5, {n}_lut6;")?;
                let (e0, d0, c0, a, b, c1, d1, e1) = (pin(0), pin(1), pin(2), pin(3), pin(4), pin(5), pin(6), pin(7));
                writeln!(v, "  assign {n}_lo = lut5({n}_w[31:0], {{{e0}, {d0}, {c0}, {b}, {a}}});")?;
                writeln!(v, "  assign {n}_hi6 = lut5({n}_w[63:32], {{{e0}, {d1}, {c1}, {b}, {a}}});")?;
                writeln!(v, "  assign {n}_hi5 = lut5({n}_w[63:32], {{{e1}, {d1}, {c1}, {b}, {a}}});")?;
                writeln!(v, "  assign {n}_lut6 = ~({e1} ? {n}_lo : {n}_hi6);")?;
                lut6 = format!("{n}_lut6");
                outs = [format!("{n}_lo"), format!("{n}_hi5")];
            }
            // The direct outputs [obs, fr: the fitted atoms behind 52 direct
            // pins in 31 LABs, tools/alm/direct.py]: lut5_powerdown =
            // LUT5_OUT drives LAB_RE 129 + 10 [k >= 5] + k % 5 from the high
            // half's 5-LUT, lut6_powerdown = LUT6_OUT the pin 5 above from
            // the 6-LUT; idle, neither is driven.
            if !arith {
                let base = 129 + if k >= 5 { 10 } else { 0 } + k % 5;
                let five = outs[1].clone();
                for (setting, val, pin, src) in [("lut5_powerdown", "LUT5_OUT", base, five), ("lut6_powerdown", "LUT6_OUT", base + 5, lut6.clone())] {
                    if get(&format!("{lut}/{setting}")) == Some(val) {
                        writeln!(d, "  wire l{x}_{y}_o{pin};")?;
                        writeln!(v, "  assign l{x}_{y}_o{pin} = {src};")?;
                    }
                }
            }
            let ce = match get(&format!("{regctrl}/xce_la_le_regctrl_mux/regclk_ens")) {
                None | Some("CLKEN0") => ces[0].clone(),
                Some("CLKEN1") => ces[1].clone(),
                Some(o) => bail!("{uid} {p}: regclk_ens {o}"),
            };
            for h in 0..2u32 {
                let rb = format!("xcw_la_le_reg{h}");
                let clr = match get(&format!("{rb}/xreg_aclr/regclr")) {
                    None | Some("CLR1") => format!("l{x}_{y}_clr1"),
                    Some("CLR2") => format!("l{x}_{y}_clr2"),
                    Some(o) => bail!("{uid} {p} {rb}: regclr {o}"),
                };
                for r in 0..2u32 {
                    let mode = get(&format!("{rb}/xreg{r}/register_comb"));
                    let input = get(&format!("{rb}/xreg{r}/reg_input")).unwrap_or("ARITH_SUMOUT");
                    let src = match (input, arith) {
                        ("ARITH_SUMOUT", true) => outs[h as usize].clone(),
                        ("LUTOUT_5LUT", true) => five[h as usize].clone(),
                        ("LUTOUT_6LUT", false) => lut6.clone(),
                        ("LUTOUT_5LUT", false) => outs[h as usize].clone(),
                        // LEIM: the ALM's pin A (half 0) or B (half 1) (README (y)).
                        ("LEIM", _) => pin(3 + h),
                        (_, _) if mode.is_none() => continue,
                        (i, _) => bail!("{uid} {p} {rb}/xreg{r}: input {i} not modelled"),
                    };
                    let o = format!("l{x}_{y}_o{}", 89 + 4 * k + 2 * h + r);
                    match mode {
                        Some("DFF_LATCH") => {
                            let clk = match get(&format!("{rb}/xlabclk_mux/regclk")) {
                                // PHI1: the LAB clock through its programmable
                                // delay (smxclk_dly), the same clock to logic
                                // (cqr: 476 flops) [inf].
                                Some("DFF_LABCLK0" | "DFF_LABCLK_PHI1_0") => clk.clone(),
                                Some("DFF_LABCLK1" | "DFF_LABCLK_PHI1_1") => clk1.clone(),
                                o => bail!("{uid} {p} {rb}: regclk {o:?}"),
                            };
                            // xreg0's ce_sel and sclr_sel serve both registers
                            // of the half [obs, Quartus' fr and cql, README (z3)].
                            let ce = if get(&format!("{rb}/xreg0/ce_sel")) == Some("CE") { ce.clone() } else { "1'b1".into() };
                            let sc = if get(&format!("{rb}/xreg0/sclr_sel")) == Some("SCLR") { sclr.clone() } else { "1'b0".into() };
                            writeln!(d, "  reg {o} = 1'b0;")?;
                            // The enable gates the synchronous clear (README (y)).
                            writeln!(v, "  always @(posedge {clk} or posedge {clr}) if ({clr}) {o} <= 1'b0; else if ({ce}) {o} <= {sc} ? 1'b0 : {src};")?;
                            nffs += 1;
                        }
                        Some("WIRE_MODE") => {
                            writeln!(d, "  wire {o};")?;
                            writeln!(v, "  assign {o} = {src};")?
                        }
                        None => {}
                        Some(m) => bail!("{uid} {p} {rb}/xreg{r}: {m} not modelled"),
                    }
                }
            }
        }
    }
    // M20Ks as 512 x 40 simple dual-port RAMs, the physical word
    // (yosys/cells_sim.v's A5_M20K40): data bit j enters at pin 57 + j
    // (j < 20) or 40 - j and leaves at 83 + j, contents are the 40 stored
    // bits as they are (README (w), (y), (z6)).
    for &(x, y) in m20ks.keys() {
        let uid = format!("cw_mm_m20k_r{y}_c{x}");
        let eab = |p: u32| sig(g.node(&format!("MEDIUM_EAB_RE:X{x}Y{y}Z0I{p}")));
        let req = |p: u32| eab(p).unwrap_or_else(|| "1'bx".into());
        // An unrouted enable is high; WE through its inverter is low.
        let en = |p: u32| eab(p).unwrap_or_else(|| "1'b1".into());
        let mut we = en(37);
        // The clock enable on pin 41 when `m20k_clkensel` selects it
        // (Quartus' CPU ROM, cqr) [obs]; idle, the clock always runs.
        let mut ce = "1'b1".to_owned();
        let (mut outreg, mut init) = (true, vec![0u64; 512]);
        for ((u, a), val) in &attr {
            if *u != uid {
                continue;
            }
            if let Some(n) = a.strip_prefix("xdigicore/xdigicorepg/xcore/xary/m20k_eram_add") {
                init[n.parse::<usize>()?] = u64::from_str_radix(val, 2)?;
            } else if a == "xsecmux/xwe0/m20k_invsel" && val == "INV_LEIM_SEL" {
                we = format!("~{we}");
            } else if a == "xsecmux/xclk0/m20k_clkensel" && val == "CLKEN_SEL" {
                ce = en(41);
            } else if a == "xsecmux/xclk0/m20k_clkensel" && val == "CLKEN_SEL_INV" {
                ce = format!("~{}", en(41));
            } else if M20K_KNOWN.contains(&a.as_str()) {
                outreg = false;
            } else if !crate::emit::M20K_SDP.iter().any(|(n, s)| a == &format!("xdigicore/{n}") && val == s) {
                bail!("{uid} {a} = {val}: M20K setting outside the model");
            }
        }
        // An unrouted data input reads 0 (a5pnr leaves 2 bits of each lane
        // open and never reads them) [inf].
        let bus = |f: &dyn Fn(u32) -> u32, n: u32, open: &str| (0..n).rev().map(|i| eab(f(i)).unwrap_or_else(|| open.into())).collect::<Vec<_>>().join(", ");
        let init_hex: String = init.iter().rev().map(|w| format!("{w:010x}")).collect();
        writeln!(d, "  wire [39:0] m{x}_{y}_q;")?;
        for j in 0..40u32 {
            writeln!(d, "  wire m{x}_{y}_q{} = m{x}_{y}_q[{j}];", 83 + j)?;
        }
        writeln!(
            v,
            "  A5_M20K40 #(.INIT(20480'h{init_hex}), .OUTREG({})) m{x}_{y} (.CLK({}), .CE({ce}), .WE({we}), .BE({{{}, {}, {}, {}}}), .RE({}), .WADDR({{{}}}), .WDATA({{{}}}), .RADDR({{{}}}), .RDATA(m{x}_{y}_q));",
            outreg as u8,
            req(77),
            en(21),
            en(22),
            en(56),
            en(55),
            en(39),
            bus(&|i| 25 + i, 9, "1'bx"),
            bus(&|j| if j < 20 { 57 + j } else { 40 - j }, 40, "1'b0"),
            bus(&|i| 52 - i, 9, "1'bx")
        )?;
    }
    // DSP blocks (README (z11)): a block's settings must be exactly one
    // multiplier mode's; its product, registered on the block's clock.
    let dsp_modes = crate::dsp::modes(db)?;
    for &(x, y) in dsps.keys() {
        let uid = format!("cw_dssm_r{y}_c{x}");
        // The proginv masks are per-pin inverters (dsp::proginv_pin), read
        // below, not part of the mode.
        let mut have: Vec<(String, String)> = attr.iter().filter(|((u, a), _)| *u == uid && !crate::dsp::is_proginv(a)).map(|((_, a), v)| (a.clone(), v.clone())).collect();
        have.sort();
        let mode = dsp_modes.iter().find(|(_, rows)| {
            let mut want: Vec<(String, String)> = rows.iter().filter(|(_, a, _)| !crate::dsp::is_proginv(a)).map(|(_, a, v)| (a.clone(), v.clone())).collect();
            want.sort();
            want == have
        });
        let Some((&m, rows)) = mode else { bail!("{uid}: DSP settings outside the model (no mode of dsp/modes.tsv matches)") };
        // Inverted pins: the bits of the block's proginv masks that differ
        // from the mode's (dsp::proginv_pin).
        let mut inv: HashMap<u16, bool> = HashMap::default();
        for (h, name) in [(false, "xcw_dssm_core/dsp_proginv_l"), (true, "xcw_dssm_core/dsp_proginv_h")] {
            let bits = |s: Option<&String>| -> Vec<bool> { s.map_or(vec![false; 54], |s| s.bytes().rev().map(|c| c == b'1').collect()) };
            let (got, base) = (bits(attr.get(&(uid.clone(), name.to_owned()))), bits(rows.iter().find(|(_, a, _)| a == name).map(|(_, _, v)| v)));
            for k in 0..54 {
                if got[k] == base[k] {
                    continue;
                }
                match crate::dsp::proginv_pin(h, k) {
                    Some(p) => {
                        inv.insert(p, true);
                    }
                    None => bail!("{uid} {name} bit {k}: an inverter on an unknown pin differs from the mode's"),
                }
            }
        }
        // An operand pin through its inverter; unrouted, it reads 1 [inf].
        let mac = |p: u16| {
            let s = sig(g.node(&format!("MP_MAC_RE:X{x}Y{y}Z0I{p}"))).unwrap_or_else(|| "1'b1".into());
            if p != crate::dsp::CLK && inv.get(&p) == Some(&true) { format!("~{s}") } else { s }
        };
        let w = m.width();
        let mut a: Vec<String> = (0..w).rev().map(|i| mac(crate::dsp::a_pin(m, i))).collect();
        let b: Vec<String> = (0..w).rev().map(|i| mac(crate::dsp::b_pin(m, i))).collect();
        if m.signed && !m.wide {
            a.insert(0, mac(crate::dsp::A_SIGN));
        }
        let n = format!("d{x}_{y}");
        let (sa, sb) = if m.signed { ("$signed", "$signed") } else { ("", "") };
        writeln!(d, "  wire [{}:0] {n}_a = {{{}}}; wire [{}:0] {n}_b = {{{}}}; reg [53:0] {n}_p = 54'b0;", a.len() - 1, a.join(", "), w - 1, b.join(", "))?;
        writeln!(v, "  always @(posedge {}) {n}_p <= {sa}({n}_a) * {sb}({n}_b);", mac(crate::dsp::CLK))?;
        for k in 0..54 {
            writeln!(d, "  wire {n}_q{} = {n}_p[{k}];", crate::dsp::p_pin(k))?;
        }
    }
    // I/O registers (README (z10)), each a flop of the pin's block: its
    // controls are active-low block pins, a signal through an inverted RPI
    // arriving inverted; an unused control is off.
    // A5_RB_MUTATE (noinv, nopre, nosclr) breaks one rule of the model on
    // purpose, to show the oracle catches it.
    let mutate = std::env::var("A5_RB_MUTATE").unwrap_or_default();
    let inv_blk: HashSet<u32> = if mutate == "noinv" { HashSet::default() } else { inverted_rpi.iter().filter_map(|&r| crate::ioreg::forward_block(g, &single, r)).collect() };
    let mut used_io: HashSet<(String, String)> = HashSet::default();
    let mut io_out: HashMap<String, (String, Option<String>)> = HashMap::default();
    let hyp = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_owned());
    for (ball, p) in &io_pins {
        let regs: Vec<&str> = [("i", "xio_gpio_ireg_4to1_mux1"), ("o", "xio_gpio_oreg_4to1_mux"), ("e", "xio_gpio_ereg_4to1_mux")].iter().filter(|(_, m)| io_has(ball, m)).map(|(r, _)| *r).collect();
        if regs.is_empty() {
            continue;
        }
        let port = port_of.get(ball).with_context(|| format!("I/O register on ball {ball}, which isn't in {pins}"))?.replace(['[', ']'], "_");
        let mut get = |sub: &str| -> Option<String> {
            let k = (p.uid.clone(), format!("{}/{sub}", p.path));
            let r = io_attr.get(&k).cloned();
            if r.is_some() {
                used_io.insert(k);
            }
            r
        };
        let blk = |r: crate::ioreg::Role| -> String {
            let Some(n) = p.block_pin(r).and_then(|n| g.node(&n)) else { return "1'bx".into() };
            match sig(Some(n)) {
                Some(s) if inv_blk.contains(&n) => format!("~{s}"),
                Some(s) => s,
                None => "1'bx".into(),
            }
        };
        // The block's register clock: the one clock source reaching any of
        // its pins [inf: Quartus' ct and iq use one clock pin per block].
        let mut clks: Vec<String> = reach.iter().filter(|(n, s)| g.name(**n).starts_with(p.prefix()) && matches!(s, Src::Port(k) if balls.iter().any(|(b, u)| u == "clock_in" && port_of.get(b) == Some(&inputs[*k].0)))).map(|(n, _)| sig(Some(*n)).unwrap()).collect();
        clks.sort();
        clks.dedup();
        let [clk] = clks.as_slice() else { bail!("{ball}: I/O register clock {clks:?}") };
        for &r in &regs {
            use crate::ioreg::Role;
            let (sub, gen, tag) = match r {
                "i" => ("xio_gpio_ireg", "xio_gpio_in_clk_rst_gen", "ireg"),
                "o" => ("xio_gpio_oreg", "xio_gpio_out_clk_rst_gen", "oreg"),
                _ => ("xio_gpio_ereg", "xio_gpio_oe_clk_rst_gen", "ereg"),
            };
            let on = |v: Option<String>| v.is_some_and(|v| v.ends_with("_ENA"));
            let ck = if r == "i" { "in" } else { "out" };
            if !on(get(&format!("{sub}/{gen}/a_rb_fr_{ck}_clk{}_ena", if r == "i" { String::new() } else { format!("_{tag}") }))) {
                bail!("{ball}: {tag} without its clock enabled");
            }
            let ce_name = if r == "i" { "ncein_ireg_ena".to_owned() } else { format!("nceout_{tag}_ena") };
            let nce = if on(get(&format!("{sub}/{gen}/a_rb_{ce_name}"))) { blk(if r == "i" { Role::InCe } else { Role::OutCe }) } else { "1'b0".into() };
            let naclr = if on(get(&format!("{sub}/{gen}/a_rb_naclr_{tag}_ena"))) { blk(Role::Aclr) } else { "1'b1".into() };
            let pre = get(&format!("{sub}/{gen}/a_rb_naclr_{tag}_sel")).is_some_and(|v| v.contains("NPRE")) && mutate != "nopre";
            let nsclr = if r == "i" && on(get(&format!("{sub}/{gen}/a_rb_nsclr_ireg_ena"))) && mutate != "nosclr" { blk(Role::InSclr) } else { "1'b1".into() };
            let dat = match r {
                "i" => port.clone(),
                "o" => blk(Role::Out),
                _ => blk(Role::Oe),
            };
            // The input register's clock inverter: Quartus sets IN_INV on
            // every input register (ct, iq: 11 of 11) [inf: back to the
            // rising edge].
            let edge = if r == "i" {
                let inv = get(&format!("{sub}/xinv_fr_in_clk/a_rb_sel")).is_some_and(|v| v == "IN_INV");
                if inv == (hyp("A5_RB_INCLK", "inv_pos") == "inv_pos") { "posedge" } else { "negedge" }
            } else {
                "posedge"
            };
            let q = format!("io_{port}_{r}");
            writeln!(d, "  reg {q} = 1'b0; wire {q}_nclr = {naclr};")?;
            writeln!(v, "  always @({edge} {clk} or negedge {q}_nclr) if (!{q}_nclr) {q} <= 1'b{}; else if (!({nce})) {q} <= !({nsclr}) ? 1'b0 : {dat};", pre as u8)?;
        }
        if regs.contains(&"i") && io96[ball] {
            get("xio_gpio_ireg_4to1_mux0/a_rb_mux_sel");
        }
        for m in ["xio_gpio_ireg_4to1_mux1", "xio_gpio_oreg_4to1_mux", "xio_gpio_ereg_4to1_mux"] {
            get(&format!("{m}/a_rb_mux_sel"));
        }
        let data = if regs.contains(&"o") { Some(format!("io_{port}_o")) } else { None };
        // The OE register drives the buffer's enable [hypothesis A5_RB_OE].
        let en = regs.contains(&"e").then(|| if hyp("A5_RB_OE", "low") == "low" { format!("~io_{port}_e") } else { format!("io_{port}_e") });
        io_out.insert(port_of[ball].clone(), (data.unwrap_or_default(), en));
    }
    // An IO96 lane's input flyby muxes: GPIO for its registered inputs
    // [obs, iq: lane 5, which has them; inf: that is what they select].
    for (k, val) in &io_attr {
        if k.1.contains("/xflyby/") && val == "GPIO" {
            used_io.insert(k.clone());
        }
    }
    for k in io_attr.keys() {
        if !used_io.contains(k) {
            bail!("{} {} = {}: I/O setting outside the model", k.0, k.1, io_attr[k]);
        }
    }
    for (name, n) in &outputs {
        // An unrouted output reads 1, through the RPI's inverter if set:
        // Quartus' constant-0 output in dq3 is exactly that [obs].
        let plain = match (sig(Some(*n)), inverted_rpi.contains(n)) {
            (Some(s), false) => s,
            (Some(s), true) => format!("~{s}"),
            (None, false) => "1'b1".into(),
            (None, true) => "1'b0".into(),
        };
        let (data, en) = io_out.get(name).cloned().unwrap_or_default();
        let data = if data.is_empty() { plain } else { data };
        match en {
            Some(e) => writeln!(v, "  assign {} = {e} ? {data} : 1'bz;", name.replace(['[', ']'], "_"))?,
            None => writeln!(v, "  assign {} = {data};", name.replace(['[', ']'], "_"))?,
        }
    }
    writeln!(v, "endmodule")?;
    let uu = used_unknown.into_inner();
    if !uu.is_empty() {
        bail!("{} used pins are reached from more than one source, e.g. {}", uu.len(), uu[0]);
    }
    std::fs::write(out, head + &d + &v)?;
    eprintln!("readback: {} DSPs, {} M20Ks, {} LABs, {} ALMs, {} flops, {} active edges, {} inputs, {} outputs", dsps.len(), m20ks.len(), labs.len(), nalms, nffs, active.len(), inputs.len(), outputs.len());
    Ok(())
}

/// Every node reached from a source in a decoded design whose name starts
/// with `prefix`, with that source (pins named by ball and role).
pub fn drivers(g: &Graph, db: &str, extract: &str, prefix: &str) -> Result<()> {
    let text = std::fs::read_to_string(extract).with_context(|| format!("reading {extract}"))?;
    let mut active: HashSet<(u32, u32)> = HashSet::default();
    let mut sources: Vec<(String, u32)> = Vec::new();
    let mut node_of: HashMap<(String, String), u32> = HashMap::default();
    for l in std::fs::read_to_string(format!("{db}/io/pin_nodes.tsv"))?.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 5 && !l.starts_with('#') && f[0] != "ball" {
            if let Some(n) = g.node(f[3]) {
                node_of.insert((f[0].to_owned(), f[2].to_owned()), n);
            }
        }
    }
    let mut srcs: BTreeMap<String, ()> = BTreeMap::new();
    for l in text.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        match f[0] {
            "pin" => {
                let (b, _) = f[1].split_once('=').context("pin")?;
                for role in ["in0", "in1", "clock"] {
                    if let Some(&n) = node_of.get(&(b.to_owned(), role.to_owned())) {
                        sources.push((format!("{b}.{role}"), n));
                    }
                }
            }
            "edge" => {
                let (a, b) = (g.node(f[1]).with_context(|| f[1].to_owned())?, g.node(f[2]).with_context(|| f[2].to_owned())?);
                active.insert((a, b));
                srcs.insert(f[1].to_owned(), ());
            }
            _ => {}
        }
    }
    // Every edge source that is a block output (LAB_RE, MEDIUM_EAB_RE, ...)
    // with no active edge into it is a source too.
    let dsts: HashSet<u32> = active.iter().map(|e| e.1).collect();
    for n in srcs.keys() {
        let m = g.node(n).unwrap();
        if !dsts.contains(&m) && ["LAB_RE", "MEDIUM_EAB_RE", "MP_MAC_RE"].contains(&g.type_name(m)) {
            sources.push((n.clone(), m));
        }
    }
    // As the readback follows edges: a hop with no bits into a node with no
    // other fanin conducts too (the spare row, the I/O blocks' REDMUXes).
    let single = g.single_fanin();
    for (s, n0) in &sources {
        let mut q = VecDeque::from([*n0]);
        let mut seen = HashSet::from_iter([*n0]);
        while let Some(m) = q.pop_front() {
            for e in g.fanout(m) {
                let d = g.dst[e];
                if (g.class[e] == PASS || (g.class[e] == NO && single[d as usize]) || active.contains(&(m, d))) && seen.insert(d) {
                    if g.name(d).starts_with(prefix) {
                        println!("{}\t{s}", g.name(d));
                    }
                    q.push_back(d);
                }
            }
        }
    }
    Ok(())
}
