//! yosys JSON (tools/a5pnr/synth.sh) → cells with constants folded.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use serde_json::Value;

/// A signal: a net or a constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sig {
    Net(u32),
    Const(bool),
}

#[derive(Clone, Debug)]
pub enum Cell {
    /// A LUT over `ins` (bit i of the index is ins[i]); `mask` has 2^len bits.
    Lut { ins: Vec<u32>, mask: u64, out: u32 },
    /// Half an ALM in arithmetic mode (yosys/cells.v): p and g over
    /// {D, C, B, A}; after folding, only C and D carry nets.
    Arith { p: u16, g: u16, c: Option<u32>, d: Option<u32>, ci: Sig, so: Option<u32>, co: Option<u32> },
    Ff { d: Sig, clk: u32, q: u32 },
    M20k { clk: u32, we: Sig, waddr: Vec<Sig>, wdata: Vec<Sig>, raddr: Vec<Sig>, rdata: Vec<Option<u32>> },
}

pub struct Port {
    pub name: String,
    pub input: bool,
    pub net: Sig,
}

pub struct Netlist {
    pub nets: Vec<String>,
    pub cells: Vec<Cell>,
    pub ports: Vec<Port>,
}

fn bits(v: &Value) -> Result<Vec<Sig>> {
    v.as_array()
        .context("connection is not a bit list")?
        .iter()
        .map(|b| match b {
            Value::Number(n) => Ok(Sig::Net(n.as_u64().context("bit")? as u32)),
            Value::String(s) if s == "0" || s == "x" || s == "z" => Ok(Sig::Const(false)),
            Value::String(s) if s == "1" => Ok(Sig::Const(true)),
            _ => bail!("bad bit {b}"),
        })
        .collect()
}

fn param(v: &Value, name: &str) -> Result<Vec<bool>> {
    let s = v.get(name).and_then(Value::as_str).with_context(|| format!("parameter {name}"))?;
    // yosys writes parameters MSB first.
    Ok(s.bytes().rev().map(|c| c == b'1').collect())
}

/// Fold constant and repeated inputs out of a truth table over `ins`.
fn fold(ins: &[Sig], table: &[bool]) -> (Vec<u32>, Vec<bool>) {
    let mut nets: Vec<u32> = Vec::new();
    for s in ins {
        if let Sig::Net(n) = s {
            if !nets.contains(n) {
                nets.push(*n);
            }
        }
    }
    let mut out = vec![false; 1 << nets.len()];
    for (j, o) in out.iter_mut().enumerate() {
        let mut idx = 0;
        for (i, s) in ins.iter().enumerate() {
            let v = match s {
                Sig::Const(c) => *c,
                Sig::Net(n) => j >> nets.iter().position(|m| m == n).unwrap() & 1 == 1,
            };
            idx |= (v as usize) << i;
        }
        *o = table[idx];
    }
    // Drop inputs the function doesn't depend on.
    let mut k = 0;
    while k < nets.len() {
        let dep = (0..out.len()).any(|j| out[j] != out[j ^ (1 << k)]);
        if dep {
            k += 1;
            continue;
        }
        out = (0..out.len() / 2).map(|j| out[(j >> k << (k + 1)) | (j & ((1 << k) - 1))]).collect();
        nets.remove(k);
    }
    (nets, out)
}

pub fn read(path: &str, top: &str) -> Result<Netlist> {
    let j: Value = serde_json::from_str(&std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?)?;
    let m = j["modules"].get(top).with_context(|| format!("no module {top}"))?;
    // A net's name: a visible register or wire name over a port's, over
    // yosys' own `$` names (simcheck sets registers by these names).
    let ports_obj = m["ports"].as_object().context("ports")?;
    let mut names: HashMap<u32, (u8, String)> = HashMap::new();
    for (n, v) in m["netnames"].as_object().context("netnames")? {
        let hidden = v.get("hide_name").and_then(Value::as_u64) == Some(1) || n.starts_with('$');
        let rank = if hidden { 2 } else if ports_obj.contains_key(n) { 1 } else { 0 };
        for (i, b) in bits(&v["bits"])?.iter().enumerate() {
            if let Sig::Net(id) = b {
                let e = names.entry(*id).or_insert((u8::MAX, String::new()));
                if rank < e.0 {
                    *e = (rank, format!("{n}[{i}]"));
                }
            }
        }
    }
    let names: HashMap<u32, String> = names.into_iter().map(|(k, (_, v))| (k, v)).collect();
    let max = names.keys().copied().max().unwrap_or(0) as usize + 1;
    let mut nets: Vec<String> = (0..max).map(|i| names.get(&(i as u32)).cloned().unwrap_or_else(|| format!("n{i}"))).collect();
    let mut cells = Vec::new();
    let mut fresh = |nets: &mut Vec<String>, name: String| -> u32 {
        nets.push(name);
        nets.len() as u32 - 1
    };
    for (name, c) in m["cells"].as_object().context("cells")? {
        let t = c["type"].as_str().unwrap_or("");
        let conn = &c["connections"];
        let p = &c["parameters"];
        let one = |port: &str| -> Result<Sig> { Ok(bits(&conn[port])?[0]) };
        let net = |s: Sig| -> Result<u32> {
            match s {
                Sig::Net(n) => Ok(n),
                Sig::Const(_) => bail!("{name}: output tied to a constant"),
            }
        };
        match t {
            "$lut" => {
                let ins = bits(&conn["A"])?;
                let table = param(p, "LUT")?;
                let (ins, t) = fold(&ins, &table);
                if ins.len() > 6 {
                    bail!("{name}: LUT with {} inputs", ins.len());
                }
                let mask = t.iter().enumerate().fold(0u64, |a, (i, &b)| a | (b as u64) << i);
                cells.push(Cell::Lut { ins, mask, out: net(one("Y")?)? });
            }
            "$_DFF_P_" => cells.push(Cell::Ff { d: one("D")?, clk: net(one("C")?)?, q: net(one("Q")?)? }),
            "A5_ARITH" => {
                let ins = [one("A")?, one("B")?, one("C")?, one("D")?];
                let (pt, gt) = (param(p, "P")?, param(p, "G")?);
                let (pn, pf) = fold(&ins, &pt);
                let (gn, gf) = fold(&ins, &gt);
                // Both functions over one input list: the union, at most two nets,
                // which go on the half's own C and D pins (README (w)).
                let mut nets_u: Vec<u32> = pn.clone();
                for n in &gn {
                    if !nets_u.contains(n) {
                        nets_u.push(*n);
                    }
                }
                if nets_u.len() > 2 {
                    bail!("{name}: arithmetic cell with {} variable inputs", nets_u.len());
                }
                let expand = |sub: &[u32], f: &[bool]| -> u16 {
                    let mut m = 0u16;
                    for idx in 0..16usize {
                        // C is index bit 2, D bit 3.
                        let mut j = 0;
                        for (k, n) in sub.iter().enumerate() {
                            let pos = nets_u.iter().position(|x| x == n).unwrap();
                            j |= (idx >> (2 + pos) & 1) << k;
                        }
                        if f[j] {
                            m |= 1 << idx;
                        }
                    }
                    m
                };
                let so = match bits(&conn["SO"]) { Ok(v) => match v.first() { Some(Sig::Net(n)) => Some(*n), _ => None }, Err(_) => None };
                let co = match bits(&conn["CO"]) { Ok(v) => match v.first() { Some(Sig::Net(n)) => Some(*n), _ => None }, Err(_) => None };
                cells.push(Cell::Arith {
                    p: expand(&pn, &pf),
                    g: expand(&gn, &gf),
                    c: nets_u.first().copied(),
                    d: nets_u.get(1).copied(),
                    ci: one("CI")?,
                    so,
                    co,
                });
            }
            "A5_M20K" => {
                let rdata = bits(&conn["RDATA"])?.into_iter().map(|s| if let Sig::Net(n) = s { Some(n) } else { None }).collect();
                cells.push(Cell::M20k {
                    clk: net(one("CLK")?)?,
                    we: one("WE")?,
                    waddr: bits(&conn["WADDR"])?,
                    wdata: bits(&conn["WDATA"])?,
                    raddr: bits(&conn["RADDR"])?,
                    rdata,
                });
            }
            _ => bail!("{name}: cell type {t} is not supported"),
        }
    }
    let mut ports = Vec::new();
    for (name, v) in m["ports"].as_object().context("ports")? {
        let input = v["direction"] == "input";
        for (i, b) in bits(&v["bits"])?.into_iter().enumerate() {
            let n = if v["bits"].as_array().map_or(0, |a| a.len()) > 1 { format!("{name}[{i}]") } else { name.clone() };
            ports.push(Port { name: n, input, net: b });
        }
    }
    // A flop with a constant D still needs a driver it can sit behind.
    for k in 0..cells.len() {
        if let Cell::Ff { d: Sig::Const(v), .. } = cells[k] {
            let n = fresh(&mut nets, format!("const{}", v as u8));
            cells.push(Cell::Lut { ins: vec![], mask: v as u64, out: n });
            if let Cell::Ff { d, .. } = &mut cells[k] {
                *d = Sig::Net(n);
            }
        }
    }
    Ok(Netlist { nets, cells, ports })
}
