//! Gate-level registers onto the flops a5pnr packs (README (y)), as
//! synth.sh's `dfflegalize` leaves them: a rising clock, an enable, a
//! synchronous clear the enable gates (`$_SDFFCE_PP0P_`) or an asynchronous
//! clear of either polarity, powering up at 0. Active-low enables and
//! synchronous clears get an inverter, which LUT mapping folds into the
//! logic driving them; a clear that acts whether or not the enable is high
//! (`$sdffe`) moves into the enable (Quartus does the same, README (y)); a
//! register that resets or powers up to 1 is kept inverted, D and Q through
//! inverters, as dfflegalize's flip_bits does.

use anyhow::{bail, Result};
use chipsmith_netlist::{Bit, CellKind, Ctrl, DffControl, Ff, FfKind, Gate, GateOp, Module, Polarity, SigBit, State, SyncEnable};
use chipsmith_synth::rewrite::{define_inits, inits, set_inits, Gates};

#[derive(Debug, Default, Clone, Copy)]
pub struct Counts {
    pub flops: usize,
    pub inverted: usize,
}

fn high(g: &mut Gates, c: Ctrl) -> Ctrl {
    match c.polarity {
        Polarity::Pos => c,
        Polarity::Neg => Ctrl::pos(g.not(c.sig)),
    }
}

pub fn map(m: &mut Module) -> Result<Counts> {
    define_inits(m);
    let init = inits(m);
    let mut g = Gates::new(m, "ff");
    let cells = std::mem::take(&mut g.m.cells);
    let mut regs = Vec::new();
    for c in cells {
        match c.kind {
            CellKind::FfGate(ff) => regs.push((c.name, ff)),
            kind => g.m.cells.push(chipsmith_netlist::Cell { kind, ..c }),
        }
    }
    let mut n = Counts { flops: regs.len(), inverted: 0 };
    for (name, ff) in regs {
        let SigBit::Net(q) = ff.q else { bail!("register {name} drives a constant") };
        let FfKind::Dff { clk, d, control } = ff.kind else {
            bail!("register {name}: only edge-triggered registers are mapped");
        };
        if clk.polarity == Polarity::Neg {
            bail!("register {name}: a falling-edge clock isn't mapped (the LAB has no clock inverter in a5pnr)");
        }
        let en = |g: &mut Gates, e: Option<Ctrl>| e.map(|e| high(g, e));
        let power_up = match init.get(&q) {
            Some(&State::One) => Some(true),
            Some(&State::Zero) => Some(false),
            _ => None,
        };
        // (control, D, the value it resets or powers up to)
        let (control, d, one) = match control {
            DffControl::Plain { en: e } => (DffControl::Plain { en: en(&mut g, e) }, d, power_up == Some(true)),
            DffControl::Async { arst, value, en: e } => {
                if power_up.is_some_and(|p| p != value) {
                    bail!("register {name}: resets to {} but powers up at {}", value as u8, !value as u8);
                }
                (DffControl::Async { arst, value: false, en: en(&mut g, e) }, d, value)
            }
            DffControl::Sync { srst, value, en: e } => {
                let srst = high(&mut g, srst);
                let e = match e {
                    SyncEnable::Always => None,
                    SyncEnable::EnableOverReset(e) => Some((high(&mut g, e), false)),
                    SyncEnable::ResetOverEnable(e) => Some((high(&mut g, e), true)),
                };
                // The enable gates the LAB's clear: a clear that acts on
                // every edge goes into the enable.
                let en = e.map(|(e, over)| if over { Ctrl::pos(g.or(e.sig, srst.sig)) } else { e });
                if power_up.is_some_and(|p| p != value) {
                    // A reset value the power-up value contradicts: the
                    // reset becomes logic in front of D (dfflegalize's way).
                    let d = if value { g.or(d, srst.sig) } else { let n = g.not(srst.sig); g.and(d, n) };
                    (DffControl::Plain { en }, d, power_up == Some(true))
                } else {
                    let en = match en {
                        None => SyncEnable::Always,
                        Some(e) => SyncEnable::EnableOverReset(e),
                    };
                    (DffControl::Sync { srst, value: false, en }, d, value)
                }
            }
            DffControl::Load { .. } | DffControl::SetClr { .. } => {
                bail!("register {name}: asynchronous loads and set-and-clear registers aren't mapped")
            }
        };
        let (d, q_ff) = if one {
            n.inverted += 1;
            let qn = SigBit::Net(g.m.new_net());
            g.add(CellKind::Gate(Gate::new(GateOp::Not, &[qn], SigBit::Net(q)).expect("arity")));
            (g.not(d), qn)
        } else {
            (d, SigBit::Net(q))
        };
        g.add(CellKind::FfGate(Ff::<Bit> { q: q_ff, kind: FfKind::Dff { clk, d, control } }));
    }
    // Every flop powers up at 0 now (inverted where the RTL said 1).
    set_inits(m, &Default::default());
    Ok(n)
}
