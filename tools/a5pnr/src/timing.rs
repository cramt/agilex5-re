//! A delay estimate of a routed design with the delay model (README (u)):
//! the longest path from a flop or an input to a flop's D, through LUTs,
//! carry chains and the routed nets, each hop costing its node type's mean.
//! No clock skew and no hold; it is the model's view, not sign-off.

use std::collections::HashMap;

use crate::netlist::{Cell, Netlist};
use crate::place::{Alm, End, Packed, ARITH_IN, LUT_IN};

pub struct Model {
    pub d: HashMap<String, f32>,
}

impl Model {
    fn get(&self, k: &str) -> f32 {
        self.d.get(k).copied().unwrap_or(0.0)
    }
}

/// Arrival times of a routed design. `sink_delay`: per (net, sink end) the
/// routed delay from the net's driver.
pub struct Timing<'a> {
    pub nl: &'a Netlist,
    pub p: &'a Packed,
    pub m: &'a Model,
    /// (ALM, pin offset) → (net, routed delay to that pin)
    pub pin_net: HashMap<(usize, u8), (u32, f32)>,
    pub memo: HashMap<(usize, u8, u8), f32>,
    /// Chain predecessor of an arithmetic half: (ALM, half) → (ALM, half).
    pub prev_half: HashMap<(usize, u8), (usize, u8)>,
}

/// M20K read data comes out 1.43 ns after the clock (mm's report_path).
const M20K_TCO: f32 = 1.429;

impl<'a> Timing<'a> {
    pub fn new(nl: &'a Netlist, p: &'a Packed, m: &'a Model, routed: &HashMap<(u32, End), f32>) -> Timing<'a> {
        let mut pin_net = HashMap::new();
        for ((net, e), d) in routed {
            if let End::In { alm, pin } = e {
                pin_net.insert((*alm, *pin), (*net, *d));
            }
        }
        let mut prev_half = HashMap::new();
        for c in &p.chains {
            let mut last: Option<(usize, u8)> = None;
            for &a in c {
                for h in 0..2u8 {
                    if let Alm::Arith { halves } = &p.alms[a] {
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
        Timing { nl, p, m, pin_net, memo: HashMap::new(), prev_half }
    }

    /// Arrival at a net's driver output.
    pub fn driver(&mut self, e: End) -> f32 {
        match e {
            End::Port(_) => 0.0,
            End::Out { ff: true, .. } => self.m.get("tco"),
            End::Out { alm, h, ff: false } => self.comb(alm, h, 0),
            End::M20k { .. } => M20K_TCO,
            _ => 0.0,
        }
    }

    fn pin(&mut self, alm: usize, pin: u8) -> Option<f32> {
        let (net, d) = *self.pin_net.get(&(alm, pin))?;
        let drv = self.p.nets.get(&net)?.0;
        Some(self.driver(drv) + d)
    }

    /// Combinational arrival at half h's output: kind 0 = LUT out or sum,
    /// 1 = carry out.
    pub fn comb(&mut self, alm: usize, h: u8, kind: u8) -> f32 {
        if let Some(&v) = self.memo.get(&(alm, h, kind)) {
            return v;
        }
        let v = match self.p.alms[alm].clone() {
            Alm::Lut { lut, .. } => {
                let n = match lut {
                    Some(l) => match &self.nl.cells[l] {
                        Cell::Lut { ins, .. } => ins.len(),
                        _ => 0,
                    },
                    None => 1,
                };
                let mut best = 0.0f32;
                for (i, letter) in "abcdef".chars().enumerate().take(n) {
                    if let Some(a) = self.pin(alm, LUT_IN[i]) {
                        best = best.max(a + self.m.get(&format!("lut:data{letter}")));
                    }
                }
                best
            }
            Alm::Arith { .. } => {
                let out = if kind == 0 { "sumout" } else { "cout" };
                let mut best = 0.0f32;
                for (k, letter) in ["c", "d"].iter().enumerate() {
                    if let Some(a) = self.pin(alm, ARITH_IN[h as usize][k]) {
                        best = best.max(a + self.m.get(&format!("arith:data{letter}>{out}")));
                    }
                }
                if let Some(&(pa, ph)) = self.prev_half.get(&(alm, h)) {
                    let cin = self.comb(pa, ph, 1);
                    best = best.max(cin + self.m.get(&format!("arith:cin>{out}")));
                }
                best
            }
        };
        self.memo.insert((alm, h, kind), v);
        v
    }

    /// The worst flop setup arrival (D path + reg_d + tsu), and where.
    pub fn worst(&mut self) -> (f32, String) {
        let mut worst = (0.0f32, String::new());
        for a in 0..self.p.alms.len() {
            let halves: Vec<(u8, bool)> = match &self.p.alms[a] {
                Alm::Lut { ff, .. } => vec![(0, ff.is_some())],
                Alm::Arith { halves } => halves.iter().enumerate().map(|(h, x)| (h as u8, x.as_ref().is_some_and(|x| x.ff.is_some()))).collect(),
            };
            for (h, ff) in halves {
                if !ff {
                    continue;
                }
                let t = self.comb(a, h, 0) + self.m.get("reg_d") + self.m.get("tsu");
                if t > worst.0 {
                    worst = (t, format!("ALM {a} half {h}"));
                }
            }
        }
        worst
    }
}
