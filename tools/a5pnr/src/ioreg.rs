//! I/O registers (README (z10)): the input, output and output-enable
//! registers of an HVIO or IO96 pin, their controls' block pins and the
//! routing tiles' programmable inverters in front of them.
//!
//! [obs, Quartus' ct and iq images]: a pin's register controls are pins of
//! its I/O block next to the pin's own data pins. On an HVIO pin, with O the
//! block pin its OE path enters (`pin_nodes.tsv` role `oe`): O − 4 the input
//! register's synchronous clear, O − 3 the asynchronous clear (or preset)
//! all three registers share, O − 2 the output and OE registers' clock
//! enable, O − 1 the input register's clock enable, O + 1 the output data.
//! On an IO96 pin, with U the block pin its output data enters (role
//! `out`): U + 2 the asynchronous clear, U + 4 the input register's clock
//! enable, U + 5 the output and OE registers' clock enable. All of them are
//! active low (`nce`, `naclr`, `nsclr`); Quartus drives an active-high
//! signal through the RPI inverter of the I/O routing tile it passes
//! (`db/sm4revb/io/rpi.tsv`). The clock is the block's register clock pin.
//! A registered input leaves on the pin's `in1` node.

use crate::det::HashMap;

use anyhow::{bail, Context, Result};

use crate::graph::{Graph, NO, PASS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    InSclr,
    Aclr,
    OutCe,
    InCe,
    Oe,
    Out,
}

/// One HVIO or IO96 pin's register block.
#[derive(Clone, Debug)]
pub struct IoPin {
    pub ball: String,
    /// The I/O block instance (`hvio_3_1`, `pio_0_0`).
    pub uid: String,
    /// The pin's register attributes' path prefix in that instance.
    pub path: String,
    pub io96: bool,
    /// The block's node prefix (`HVIOWR_RE:X121Y30Z500I`).
    prefix: String,
    /// O (HVIO) or U (IO96).
    base: u32,
    /// The OE data block pin (IO96: from pin_nodes, not U-relative).
    oe: u32,
}

impl IoPin {
    /// The block's node name prefix: its clock pins are among these.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn block_pin(&self, r: Role) -> Option<String> {
        let o = self.base as i64;
        let i = if self.io96 {
            match r {
                Role::Aclr => o + 2,
                Role::InCe => o + 4,
                Role::OutCe => o + 5,
                Role::Out => o,
                Role::Oe => self.oe as i64,
                // Not seen on IO96 [unk].
                Role::InSclr => return None,
            }
        } else {
            match r {
                Role::InSclr => o - 4,
                Role::Aclr => o - 3,
                Role::OutCe => o - 2,
                Role::InCe => o - 1,
                Role::Oe => o,
                Role::Out => o + 1,
            }
        };
        Some(format!("{}{i}", self.prefix))
    }
}

/// The block pin a fabric node's hops with no choice lead to (PASS, or a
/// bitless hop into a node with no other fanin): the I/O block pin an RPI
/// boundary node feeds.
pub fn forward_block(g: &Graph, single: &[bool], mut n: u32) -> Option<u32> {
    for _ in 0..8 {
        let t = g.type_name(n);
        if t == "HVIOWR_RE" || t == "IO96_RE" {
            return Some(n);
        }
        let next: Vec<u32> = g.fanout(n).filter(|&e| g.class[e] == PASS || (g.class[e] == NO && single[g.dst[e] as usize])).map(|e| g.dst[e]).collect();
        let [m] = next.as_slice() else { return None };
        n = *m;
    }
    None
}

/// Every ball's register block, from db/sm4revb/io/pins.tsv (pad names)
/// and pin_nodes.tsv (the oe and out boundary nodes).
pub fn pins(g: &Graph, db: &str) -> Result<HashMap<String, IoPin>> {
    let single = g.single_fanin();
    let mut nodes: HashMap<(String, String), u32> = HashMap::default();
    for l in std::fs::read_to_string(format!("{db}/io/pin_nodes.tsv"))?.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 5 && !l.starts_with('#') && f[0] != "ball" {
            if let Some(n) = g.node(f[3]) {
                nodes.insert((f[0].to_owned(), f[2].to_owned()), n);
            }
        }
    }
    let mut out = HashMap::default();
    for l in std::fs::read_to_string(format!("{db}/io/pins.tsv"))?.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 7 || l.starts_with('#') || f[0] == "ball" {
            continue;
        }
        let (ball, uid, pad) = (f[0], f[1], f[2]);
        let io96 = uid.starts_with("pio");
        let path = if io96 {
            // P<n>X..: lane n / 12, pin n % 12.
            let n: u32 = pad.strip_prefix('P').and_then(|r| r.split('X').next()).context("IO96 pad")?.parse()?;
            format!("x0/xiophyadaptwr_{}/x_gpio_{}", n / 12, n % 12)
        } else {
            let p: Vec<&str> = pad.split('_').collect();
            let [_, b, gg, k, ..] = p.as_slice() else { bail!("HVIO pad {pad}") };
            format!("xx20_u{}/xx4_u{gg}/xx1_u{k}/xiogpio_reg", b.trim_start_matches('B'))
        };
        let blk = |role: &str| nodes.get(&(ball.to_owned(), role.to_owned())).and_then(|&n| forward_block(g, &single, n));
        let (Some(oe), Some(outb)) = (blk("oe"), blk("out")) else { continue };
        let name = g.name(oe);
        let prefix = name[..name.rfind('I').context("node name")? + 1].to_owned();
        if g.name(outb)[..prefix.len()] != prefix {
            // The output data on another block than the OE (HVIO B5, IO96
            // DN25): IO96 counts from the output data, HVIO from the OE.
            if !io96 {
                continue;
            }
        }
        let (base, prefix) = if io96 {
            let n = g.name(outb);
            (g.i[outb as usize], n[..n.rfind('I').unwrap() + 1].to_owned())
        } else {
            (g.i[oe as usize], prefix)
        };
        out.insert(ball.to_owned(), IoPin { ball: ball.to_owned(), uid: uid.to_owned(), path, io96, prefix, base, oe: g.i[oe as usize] });
    }
    Ok(out)
}

/// The RPI nodes of `db/sm4revb/io/rpi.tsv`, keyed by (tile, attribute path).
pub fn rpis(g: &Graph, db: &str) -> Result<HashMap<(String, String), u32>> {
    let mut m = HashMap::default();
    for l in std::fs::read_to_string(format!("{db}/io/rpi.tsv")).context("reading io/rpi.tsv")?.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() == 3 && !l.starts_with('#') && f[0] != "uid" {
            if let Some(n) = g.node(f[2]) {
                m.insert((f[0].to_owned(), f[1].to_owned()), n);
            }
        }
    }
    Ok(m)
}
