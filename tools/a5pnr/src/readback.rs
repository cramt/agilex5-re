//! The netlist an image holds, rebuilt from `a5db extract`'s decode of it
//! and written as Verilog, for simulation against the source design
//! (README (w)). Nothing comes from a5pnr's own intent: connectivity is the
//! image's decoded mux edges plus the graph's pass-through edges, cells are
//! the decoded LAB attributes and LUT masks, ports are the design file's
//! pins at their boundary nodes (README (t)).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};

use crate::graph::{Graph, PASS};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum Src {
    /// LAB (x, y) output pin.
    Lab(u32, u32, u32),
    /// Top-level input port k.
    Port(usize),
    /// M20K (x, y) output pin.
    M20k(u32, u32, u32),
}

pub fn run(g: &Graph, db: &str, extract: &str, pins: &str, top: &str, out: &str) -> Result<()> {
    let text = std::fs::read_to_string(extract).with_context(|| format!("reading {extract}"))?;
    let mut active: HashSet<(u32, u32)> = HashSet::new();
    let mut attr: HashMap<(String, String), String> = HashMap::new();
    let mut balls: Vec<(String, String)> = Vec::new();
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
            "attr" | "direct" => {
                attr.insert((f[1].to_owned(), f[2].to_owned()), f[3].to_owned());
            }
            "bit" | "raw" => bail!("{extract}: the image has bits outside the design vocabulary: {l}"),
            _ => {}
        }
    }
    // Port names by ball (pins.txt) and boundary nodes (io/pin_nodes.tsv).
    let mut port_of: HashMap<String, String> = HashMap::new();
    for l in std::fs::read_to_string(pins)?.lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() >= 2 && !f[0].starts_with('#') {
            port_of.insert(f[1].to_owned(), f[0].to_owned());
        }
    }
    let mut node_of: HashMap<(String, String), u32> = HashMap::new();
    let mut io96: HashMap<String, bool> = HashMap::new();
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
    for (b, u) in &balls {
        let name = port_of.get(b).with_context(|| format!("ball {b} isn't in {pins}"))?.clone();
        let role = match u.as_str() {
            "clock_in" => "clock",
            "out" => "out",
            "in0" | "in1" => u.as_str(),
            _ => if io96[b] { "in0" } else { "in1" },
        };
        let n = *node_of.get(&(b.clone(), role.to_owned())).with_context(|| format!("{b} {role}"))?;
        if role == "out" { outputs.push((name, n)) } else { inputs.push((name, n)) }
    }
    // Used LABs and their ALMs.
    let mut labs: BTreeMap<(u32, u32), ()> = BTreeMap::new();
    let mut m20ks: BTreeMap<(u32, u32), ()> = BTreeMap::new();
    for (u, _) in attr.keys() {
        if let Some(rest) = u.strip_prefix("cw_mm_m20k_r") {
            let (y, x) = rest.split_once("_c").context("uid")?;
            m20ks.insert((x.parse()?, y.parse()?), ());
        }
        if let Some(rest) = u.strip_prefix("cw_la_lab_r") {
            let (y, x) = rest.split_once("_c").context("uid")?;
            labs.insert((x.parse()?, y.parse()?), ());
        }
    }
    // Every source's reach over conducting edges: decoded mux edges and the
    // graph's pass-through/hardwired ones.
    let mut sources: Vec<(Src, u32)> = Vec::new();
    for &(x, y) in labs.keys() {
        for pin in 89..=128 {
            if let Some(n) = g.node(&format!("LAB_RE:X{x}Y{y}Z0I{pin}")) {
                sources.push((Src::Lab(x, y, pin), n));
            }
        }
    }
    for (k, (_, n)) in inputs.iter().enumerate() {
        sources.push((Src::Port(k), *n));
    }
    for &(x, y) in m20ks.keys() {
        for k in 0..32u32 {
            if let Some(n) = g.node(&format!("MEDIUM_EAB_RE:X{x}Y{y}Z0I{}", 83 + k)) {
                sources.push((Src::M20k(x, y, 83 + k), n));
            }
        }
    }
    let mut reach: HashMap<u32, Src> = HashMap::new();
    let mut shorts = 0;
    for &(s, n0) in &sources {
        let mut q = VecDeque::from([n0]);
        let mut seen = HashSet::from([n0]);
        while let Some(m) = q.pop_front() {
            for e in g.fanout(m) {
                let d = g.dst[e];
                if (g.class[e] == PASS || active.contains(&(m, d))) && seen.insert(d) {
                    match reach.insert(d, s) {
                        Some(o) if o != s => {
                            shorts += 1;
                            eprintln!("readback: {} reached from {o:?} and {s:?}", g.name(d));
                        }
                        _ => {}
                    }
                    q.push_back(d);
                }
            }
        }
    }
    if shorts > 0 {
        bail!("{shorts} nodes driven by more than one source");
    }
    let sig = |n: Option<u32>| -> String {
        match n.and_then(|n| reach.get(&n)) {
            Some(Src::Lab(x, y, p)) => format!("l{x}_{y}_o{p}"),
            Some(Src::Port(k)) => inputs[*k].0.replace(['[', ']'], "_"),
            Some(Src::M20k(x, y, p)) => format!("m{x}_{y}_q[{}]", p - 83),
            None => "1'bx".into(),
        }
    };
    let lab_node = |x: u32, y: u32, pin: u32| g.node(&format!("LAB_RE:X{x}Y{y}Z0I{pin}"));
    let mut head = String::new();
    let mut v = String::new();
    writeln!(head, "// Rebuilt by a5pnr readback from {extract} (README (w)).")?;
    let ports: Vec<String> = inputs.iter().map(|p| format!("input wire {}", p.0.replace(['[', ']'], "_"))).chain(outputs.iter().map(|p| format!("output wire {}", p.0.replace(['[', ']'], "_")))).collect();
    writeln!(head, "module {top}_readback ({});", ports.join(", "))?;
    // LUTs as mux trees: an unknown select bit gives a known output when
    // both halves agree, as in silicon (a vector index with an x is all x).
    writeln!(head, "  function automatic lut4(input [15:0] m, input [3:0] i); reg [7:0] a; reg [3:0] b; reg [1:0] c;")?;
    writeln!(head, "    begin a = i[3] ? m[15:8] : m[7:0]; b = i[2] ? a[7:4] : a[3:0]; c = i[1] ? b[3:2] : b[1:0]; lut4 = i[0] ? c[1] : c[0]; end")?;
    writeln!(head, "  endfunction")?;
    writeln!(head, "  function automatic lut6(input [63:0] m, input [5:0] i); reg [15:0] a;")?;
    writeln!(head, "    begin a = i[5] ? (i[4] ? m[63:48] : m[47:32]) : (i[4] ? m[31:16] : m[15:0]); lut6 = lut4(a, i[3:0]); end")?;
    writeln!(head, "  endfunction")?;
    let alm = |k: u32| if k < 5 { format!("xtophalf/xale{k}") } else { format!("xbothalf/xale{}", k - 5) };
    // Declarations first: cells refer to each other in any order.
    let mut d = String::new();
    let mut nffs = 0;
    let mut nalms = 0;
    for &(x, y) in labs.keys() {
        let uid = format!("cw_la_lab_r{y}_c{x}");
        let clk = sig(lab_node(x, y, 83));
        for k in 0..10u32 {
            let p = alm(k);
            let mask = attr.get(&(uid.clone(), format!("{p}/xcw_la_le_lut/lut_mask")));
            let get = |a: &str| attr.get(&(uid.clone(), format!("{p}/{a}"))).map(String::as_str);
            let used = (0..2).any(|h| (0..2).any(|r| get(&format!("xcw_la_le_reg{h}/xreg{r}/register_comb")).is_some()));
            if mask.is_none() && !used {
                continue;
            }
            nalms += 1;
            let stored: Vec<u8> = mask.map_or(vec![1; 64], |m| m.bytes().rev().map(|c| c - b'0').collect());
            let pin = |q: u32| sig(lab_node(x, y, 8 * k + q));
            let n = format!("l{x}_{y}_a{k}");
            let arith = get("xcw_la_le_lut/adder_mode") == Some("ENABLE_ADDER");
            let mut outs: [String; 2] = [String::new(), String::new()];
            if arith {
                // Half h: p over {D, C, B, A} at 32h, g at 32h + 16 (README (w)).
                let (a, b) = (pin(3), pin(4));
                for h in 0..2u32 {
                    let (c, d_in) = if h == 0 { (pin(2), pin(1)) } else { (pin(5), pin(6)) };
                    let bits = |off: usize| -> String { (0..16).rev().map(|i| (b'0' + stored[off + i]) as char).collect() };
                    let cin = if h == 0 {
                        // From the previous ALM, or ALM 9 of the LAB above.
                        if k == 0 { if labs.contains_key(&(x, y + 1)) { format!("l{x}_{}_a9_co1", y + 1) } else { "1'bx".into() } } else { format!("l{x}_{y}_a{}_co1", k - 1) }
                    } else {
                        format!("{n}_co0")
                    };
                    writeln!(d, "  wire [15:0] {n}_p{h} = 16'b{}, {n}_g{h} = 16'b{};", bits(32 * h as usize), bits(32 * h as usize + 16))?;
                    writeln!(d, "  wire [3:0] {n}_i{h}; wire {n}_pp{h}, {n}_co{h}, {n}_s{h};")?;
                    writeln!(v, "  assign {n}_i{h} = {{{dd}, {c}, {b}, {a}}};", dd = d_in)?;
                    writeln!(v, "  assign {n}_pp{h} = lut4({n}_p{h}, {n}_i{h});")?;
                    writeln!(v, "  assign {n}_co{h} = {n}_pp{h} ? {cin} : lut4({n}_g{h}, {n}_i{h});")?;
                    writeln!(v, "  assign {n}_s{h} = {cin} ^ {n}_pp{h};")?;
                    outs[h as usize] = format!("{n}_s{h}");
                }
            } else {
                // W[i] = ¬stored[i ^ 31] over {F, E, D, C, B, A} (README (h)).
                let w: String = (0..64).rev().map(|i| if stored[i ^ 31] == 1 { '0' } else { '1' }).collect();
                writeln!(d, "  wire [63:0] {n}_w = 64'b{w}; wire {n}_lut;")?;
                writeln!(v, "  assign {n}_lut = lut6({n}_w, {{{}, {}, {}, {}, {}, {}}});", pin(7), pin(0), pin(1), pin(2), pin(4), pin(3))?;
                outs[0] = format!("{n}_lut");
            }
            for h in 0..2u32 {
                for r in 0..2u32 {
                    let mode = get(&format!("xcw_la_le_reg{h}/xreg{r}/register_comb"));
                    let input = get(&format!("xcw_la_le_reg{h}/xreg{r}/reg_input")).unwrap_or("ARITH_SUMOUT");
                    let src = match (input, arith) {
                        ("ARITH_SUMOUT", true) => outs[h as usize].clone(),
                        ("LUTOUT_6LUT", false) if h == 0 => outs[0].clone(),
                        (_, _) if mode.is_none() => continue,
                        (i, _) => bail!("{uid} {p} reg{h}/xreg{r}: input {i} not modelled"),
                    };
                    let o = format!("l{x}_{y}_o{}", 89 + 4 * k + 2 * h + r);
                    match mode {
                        Some("DFF_LATCH") => {
                            if get(&format!("xcw_la_le_reg{h}/xlabclk_mux/regclk")) != Some("DFF_LABCLK0") {
                                bail!("{uid} {p} reg{h}: not on LAB clock 0");
                            }
                            writeln!(d, "  reg {o} = 1'b0;")?;
                            writeln!(v, "  always @(posedge {clk}) {o} <= {src};")?;
                            nffs += 1;
                        }
                        Some("WIRE_MODE") => {
                            writeln!(d, "  wire {o};")?;
                            writeln!(v, "  assign {o} = {src};")?
                        }
                        None => {}
                        Some(m) => bail!("{uid} {p} reg{h}/xreg{r}: {m} not modelled"),
                    }
                }
            }
        }
    }
    // M20Ks in their 512 x 32 simple dual-port use, pins as README (w);
    // the model is tools/a5pnr/yosys/cells_sim.v's A5_M20K.
    for &(x, y) in m20ks.keys() {
        let eab = |p: u32| sig(g.node(&format!("MEDIUM_EAB_RE:X{x}Y{y}Z0I{p}")));
        let bus = |f: &dyn Fn(u32) -> u32, n: u32| (0..n).rev().map(|i| eab(f(i))).collect::<Vec<_>>().join(", ");
        writeln!(d, "  wire [31:0] m{x}_{y}_q;")?;
        writeln!(
            v,
            "  A5_M20K m{x}_{y} (.CLK({}), .WE(1'b1), .WADDR({{{}}}), .WDATA({{{}}}), .RADDR({{{}}}), .RDATA(m{x}_{y}_q));",
            eab(77),
            bus(&|i| 25 + i, 9),
            bus(&|k| if k < 20 { 57 + k } else { 40 - k }, 32),
            bus(&|i| 52 - i, 9)
        )?;
    }
    for (name, n) in &outputs {
        writeln!(v, "  assign {} = {};", name.replace(['[', ']'], "_"), sig(Some(*n)))?;
    }
    writeln!(v, "endmodule")?;
    std::fs::write(out, head + &d + &v)?;
    eprintln!("readback: {} M20Ks, {} LABs, {} ALMs, {} flops, {} active edges, {} inputs, {} outputs", m20ks.len(), labs.len(), nalms, nffs, active.len(), inputs.len(), outputs.len());
    Ok(())
}
