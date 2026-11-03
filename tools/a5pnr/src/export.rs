//! a5pnr's packed and placed design as a Quartus Pro project (README (z4)):
//! a WYSIWYG netlist of `tennm_lcell_comb` and `tennm_ff` atoms, one per LUT,
//! arithmetic half and flop, each with a location assignment at the ALM half
//! and register a5pnr chose. Quartus routes it itself: Quartus Pro takes no
//! routing constraints (its ECO `reroute` rips a route up and ignores a
//! path), so quartus_sta times a5pnr's placement on Quartus' routes.
//!
//! Locations [obs, fr's and cql's atoms]: ALM k's half h LUT is
//! `LABCELL_X<x>_Y<y>_N<6k + 3h>`, its register r `FF_X<x>_Y<y>_N<6k + 3h +
//! 1 + r>`. An atom's ports are the ALM's pins (A, B, C_h, D_h, E_h; a
//! 6-LUT's E is E0 and F E1), so its WYSIWYG mask is the function over pins.

use std::collections::BTreeMap;
use crate::det::HashMap;
use std::fmt::Write as _;

use anyhow::{bail, Result};

use crate::netlist::{Cell, Netlist, Sig};
use crate::pack::{half_function, widen, HalfLut, Logic, Packed, Slot, PIN_A, PIN_B};
use crate::place::{Lab, Placement};

fn sig(s: Sig) -> String {
    match s {
        Sig::Net(n) => format!("n{n}"),
        Sig::Const(b) => format!("1'b{}", b as u8),
    }
}

/// The atom port a half's pin is on: A, B shared, then the half's C, D, E.
fn half_port(h: usize, pin: u8) -> char {
    match pin {
        PIN_A => 'a',
        PIN_B => 'b',
        p => match crate::pack::HALF_PINS[h].iter().position(|&q| q == p) {
            Some(0) => 'c',
            Some(1) => 'd',
            _ => 'e',
        },
    }
}

/// Port name and bit of a netlist port ("LED[3]" → ("LED", Some(3))).
fn split(name: &str) -> (&str, Option<usize>) {
    match name.split_once('[') {
        Some((b, i)) => (b, i.trim_end_matches(']').parse().ok()),
        None => (name, None),
    }
}

/// Writes `<dir>/<top>.v`, `.qsf`, `.sdc` and `.qpf`. `pins`: port name →
/// (ball, I/O standard as in the pins file).
#[allow(clippy::too_many_arguments)]
pub fn write(
    dir: &str,
    top: &str,
    nl: &Netlist,
    p: &Packed,
    labs: &[Lab],
    pl: &Placement,
    pins: &HashMap<String, (String, String)>,
    io: &[crate::ioreg::IoRegs],
) -> Result<()> {
    if !p.plls.is_empty() {
        bail!("quartus export: IOPLLs aren't exported yet");
    }
    std::fs::create_dir_all(dir)?;
    let mut v = String::new();
    let mut loc = String::new();
    // Ports as the RTL declares them: vectors by base name.
    let mut ports: BTreeMap<&str, (bool, Option<usize>)> = BTreeMap::new();
    for port in &nl.ports {
        let (b, i) = split(&port.name);
        let e = ports.entry(b).or_insert((port.input, None));
        if let Some(i) = i {
            e.1 = Some(e.1.map_or(i, |m: usize| m.max(i)));
        }
    }
    writeln!(v, "// a5pnr's placement of {top} as Quartus atoms (tools/a5pnr/src/export.rs).")?;
    let decl: Vec<String> = ports
        .iter()
        .map(|(b, (input, w))| format!("{} wire {}{b}", if *input { "input" } else { "output" }, w.map_or(String::new(), |m| format!("[{m}:0] "))))
        .collect();
    writeln!(v, "module {top} (\n    {}\n);", decl.join(",\n    "))?;
    // Every net is a wire n<id> (unused ones cost Quartus a warning); ports
    // connect to theirs.
    for n in 0..nl.nets.len() {
        writeln!(v, "  wire n{n};")?;
    }
    // An output an OE register drives is the tristate its $_TBUF_ makes.
    let tbuf: HashMap<u32, (Sig, Sig)> = nl
        .cells
        .iter()
        .filter_map(|c| match c {
            Cell::Tbuf { a, e, y } => Some((*y, (*a, *e))),
            _ => None,
        })
        .collect();
    for port in &nl.ports {
        let (b, i) = split(&port.name);
        let pn = i.map_or(b.to_owned(), |i| format!("{b}[{i}]"));
        match (port.input, port.net) {
            (true, Sig::Net(n)) => writeln!(v, "  assign n{n} = {pn};")?,
            (true, Sig::Const(_)) => {}
            (false, Sig::Net(n)) if tbuf.contains_key(&n) => {
                let (a, e) = tbuf[&n];
                writeln!(v, "  assign {pn} = {} ? {} : 1'bz;", sig(e), sig(a))?
            }
            (false, s) => writeln!(v, "  assign {pn} = {};", sig(s))?,
        }
    }
    let write_ff = |v: &mut String, name: &str, f: &crate::netlist::Ff| -> Result<()> {
        let mut conns = vec![format!(".d({})", sig(f.d)), format!(".clk(n{})", f.clk), format!(".q(n{})", f.q)];
        // tennm_ff's synchronous clear acts only while enabled
        // (sclr_over_ena = "false"); a clear that wins over the
        // enable also enables.
        match (f.ce, f.sclr) {
            (Some(e), Some(c)) if f.sclr_first => conns.push(format!(".ena(n{e} | n{c})")),
            (Some(e), _) => conns.push(format!(".ena(n{e})")),
            _ => {}
        }
        if let Some(c) = f.sclr {
            conns.push(format!(".sclr(n{c})"));
        }
        if let Some(c) = f.aclr {
            conns.push(format!(".clrn({}n{})", if c.active_low { "" } else { "~" }, c.net));
        }
        writeln!(v, "  tennm_ff #(.power_up(\"low\")) {name} ({});", conns.join(", "))?;
        Ok(())
    };
    let lut = |v: &mut String, name: &str, mask: u64, ports: &[(char, u32)], out: u32| -> Result<()> {
        let mut conns: Vec<String> = ports.iter().map(|(c, n)| format!(".data{c}(n{n})")).collect();
        conns.push(format!(".combout(n{out})"));
        writeln!(v, "  tennm_lcell_comb #(.lut_mask(64'h{mask:016x})) {name} ({});", conns.join(", "))?;
        Ok(())
    };
    for (a, alm) in p.alms.iter().enumerate() {
        let (l, k) = pl.at[a];
        let (x, y) = (labs[l].x, labs[l].y);
        let k = k as u32;
        let site = |h: usize| format!("LABCELL_X{x}_Y{y}_N{}", 6 * k + 3 * h as u32);
        match &alm.logic {
            // One RAM bit per half (README (z9)) as Quartus' own MLAB atom
            // (eda/fv_lib/verilog/tennm/tennm_mlab_cell.v): a 32 x 1 RAM,
            // write on clk0 while ena0, read unregistered.
            Logic::Ram { halves } => {
                for (h, c) in halves.iter().enumerate() {
                    let Some(c) = c else { continue };
                    let Cell::Mlab(m) = &nl.cells[*c] else { unreachable!() };
                    let bus = |s: &[Sig]| format!("{{{}}}", s.iter().rev().map(|&b| sig(b)).collect::<Vec<_>>().join(", "));
                    let name = format!("a{a}_r{h}");
                    let out = m.rdata.map_or(String::new(), |n| format!(", .portbdataout(n{n})"));
                    writeln!(
                        v,
                        "  tennm_mlab_cell #(.logical_ram_name(\"{name}\"), .logical_ram_depth(32), .logical_ram_width(1), .first_address(0), .last_address(31), .first_bit_number(0), .data_width(1), .address_width(5), .port_b_data_out_clock(\"none\"), .mem_init0(\"{:08X}\")) {name} (.portadatain({}), .portaaddr({}), .portbaddr({}), .clk0(n{}), .ena0({}){out});",
                        m.init,
                        sig(m.wdata),
                        bus(&m.waddr),
                        bus(&m.raddr),
                        m.clk,
                        sig(m.we)
                    )?;
                    writeln!(loc, "set_location_assignment MLABCELL_X{x}_Y{y}_N{} -to {name}", 6 * k + 3 * h as u32)?;
                }
            }
            Logic::Six { cell, pos } => {
                let Cell::Lut { ins, mask, out } = &nl.cells[*cell] else { unreachable!() };
                let w = widen(*mask, pos);
                let ports: Vec<(char, u32)> = ins.iter().zip(pos).map(|(&n, &q)| (b"abcdef"[q as usize] as char, n)).collect();
                // The atom sits in the half whose register slots read it.
                let h = (0..4).find(|&s| matches!(alm.slots[s], Some(Slot::Ff { src: crate::pack::Src::Lut, .. }) | Some(Slot::Wire { src: crate::pack::Src::Lut }))).map_or(0, |s| s / 2);
                let name = format!("a{a}_l");
                lut(&mut v, &name, w, &ports, *out)?;
                writeln!(loc, "set_location_assignment {} -to {name}", site(h))?;
            }
            Logic::Five { halves } => {
                for (h, half) in halves.iter().enumerate() {
                    let Some(half) = half else { continue };
                    let HalfLut::Cell(c) = half.lut else { continue };
                    let Cell::Lut { ins, mask, out } = &nl.cells[c] else { unreachable!() };
                    let f = half_function(h, *mask, &half.pins) as u64;
                    let ports: Vec<(char, u32)> = ins.iter().zip(&half.pins).map(|(&n, &q)| (half_port(h, q), n)).collect();
                    let name = format!("a{a}_l{h}");
                    lut(&mut v, &name, f | f << 32, &ports, *out)?;
                    writeln!(loc, "set_location_assignment {} -to {name}", site(h))?;
                }
            }
            Logic::Arith { halves } => {
                for (h, c) in halves.iter().enumerate() {
                    let Some(c) = c else { continue };
                    let Cell::Arith { p: pm, g, c: cn, d, ci, so, co } = &nl.cells[*c] else { unreachable!() };
                    // A chain's first cin is left open, as Quartus' are.
                    let mut conns = if *ci == Sig::Const(false) { vec![] } else { vec![format!(".cin({})", sig(*ci))] };
                    if let Some(n) = cn {
                        conns.push(format!(".datac(n{n})"));
                    }
                    if let Some(n) = d {
                        conns.push(format!(".datad(n{n})"));
                    }
                    if let Some(n) = so {
                        conns.push(format!(".sumout(n{n})"));
                    }
                    if let Some(n) = co {
                        conns.push(format!(".cout(n{n})"));
                    }
                    let mask = (*pm as u64) | (*g as u64) << 16;
                    let name = format!("a{a}_s{h}");
                    writeln!(v, "  tennm_lcell_comb #(.lut_mask(64'h{mask:016x})) {name} ({});", conns.join(", "))?;
                    writeln!(loc, "set_location_assignment {} -to {name}", site(h))?;
                }
            }
        }
        for s in 0..4 {
            let Some(Slot::Ff { ff, .. }) = alm.slots[s] else { continue };
            let Cell::Ff(f) = &nl.cells[ff] else { unreachable!() };
            let name = format!("a{a}_f{s}");
            write_ff(&mut v, &name, f)?;
            writeln!(loc, "set_location_assignment FF_X{x}_Y{y}_N{} -to {name}", 6 * k + 3 * (s as u32 / 2) + 1 + s as u32 % 2)?;
        }
    }
    // M20Ks as a5pnr's model (yosys/cells_sim.v), which Quartus infers and
    // places itself (one block; its inferred atom's name isn't known here).
    for (m, &c) in p.m20ks.iter().enumerate() {
        let Cell::M20k(r) = &nl.cells[c] else { unreachable!() };
        let bus = |s: &[Sig]| format!("{{{}}}", s.iter().rev().map(|&b| sig(b)).collect::<Vec<_>>().join(", "));
        let init: String = r.init.iter().rev().map(|w| format!("{w:08x}")).collect();
        let rdata: Vec<String> = r.rdata.iter().rev().enumerate().map(|(i, o)| o.map_or(format!("u{m}_{i}"), |n| format!("n{n}"))).collect();
        for (i, o) in r.rdata.iter().rev().enumerate() {
            if o.is_none() {
                writeln!(v, "  wire u{m}_{i};")?;
            }
        }
        writeln!(
            v,
            "  a5_m20k #(.INIT(16384'h{init}), .OUTREG({})) m{m} (.CLK(n{}), .WE({}), .BE({}), .RE({}), .WADDR({}), .WDATA({}), .RADDR({}), .RDATA({{{}}}));",
            r.outreg as u8,
            r.clk,
            sig(r.we),
            bus(&r.be),
            sig(r.re),
            bus(&r.waddr),
            bus(&r.wdata),
            bus(&r.raddr),
            rdata.join(", ")
        )?;
    }
    // I/O registers as flops Quartus puts in the pin's I/O cell itself
    // (FAST_*_REGISTER below): no location of their own.
    let mut fast = String::new();
    for r in io {
        let port = &nl.ports[r.port].name;
        for (f, what) in [(r.input, "FAST_INPUT_REGISTER"), (r.output, "FAST_OUTPUT_REGISTER"), (r.oe, "FAST_OUTPUT_ENABLE_REGISTER")] {
            let Some(f) = f else { continue };
            let Cell::Ff(c) = &nl.cells[f] else { unreachable!() };
            write_ff(&mut v, &format!("io{f}"), c)?;
            writeln!(fast, "set_instance_assignment -name {what} ON -to {port}")?;
        }
    }
    // DSP blocks as a5pnr's model (README (z11)), an operand product into
    // the output register, which Quartus infers and places itself, as the
    // M20Ks.
    for (i, d) in p.dsps.iter().enumerate() {
        let Cell::Mul { mode, a, b, p: prod } = &nl.cells[d.cell] else { unreachable!() };
        let bus = |s: &[Sig]| format!("{{{}}}", s.iter().rev().map(|&b| sig(b)).collect::<Vec<_>>().join(", "));
        let q: Vec<String> = (0..prod.len())
            .rev()
            .map(|k| match d.regs[k] {
                Some(f) => match &nl.cells[f] {
                    Cell::Ff(f) => format!("n{}", f.q),
                    _ => unreachable!(),
                },
                None => {
                    format!("q{i}_{k}")
                }
            })
            .collect();
        for k in 0..prod.len() {
            if d.regs[k].is_none() {
                writeln!(v, "  wire q{i}_{k};")?;
            }
        }
        writeln!(
            v,
            "  a5_mul #(.W({}), .PW({}), .SIGNED({})) d{i} (.CLK(n{}), .A({}), .B({}), .P({{{}}}));",
            mode.width(),
            prod.len(),
            mode.signed as u8,
            d.clk,
            bus(a),
            bus(b),
            q.join(", ")
        )?;
    }
    writeln!(v, "endmodule")?;
    if !p.m20ks.is_empty() {
        v.push_str(M20K_RTL);
    }
    if !p.dsps.is_empty() {
        v.push_str(MUL_RTL);
    }
    std::fs::write(format!("{dir}/{top}.v"), v)?;

    let mut q = String::new();
    writeln!(q, "set_global_assignment -name FAMILY \"Agilex 5\"")?;
    writeln!(q, "set_global_assignment -name DEVICE A5EB013BB23BE4SCS")?;
    writeln!(q, "set_global_assignment -name TOP_LEVEL_ENTITY {top}")?;
    writeln!(q, "set_global_assignment -name VERILOG_FILE {top}.v")?;
    writeln!(q, "set_global_assignment -name SDC_FILE {top}.sdc")?;
    writeln!(q, "set_global_assignment -name DEVICE_INITIALIZATION_CLOCK OSC_CLK_1_125MHZ")?;
    writeln!(q, "set_global_assignment -name PROJECT_OUTPUT_DIRECTORY output_files")?;
    writeln!(q, "set_global_assignment -name PWRMGT_VOLTAGE_OUTPUT_FORMAT \"LINEAR FORMAT\"")?;
    writeln!(q, "set_global_assignment -name PWRMGT_LINEAR_FORMAT_N \"-12\"")?;
    writeln!(q, "set_global_assignment -name NUM_PARALLEL_PROCESSORS 2")?;
    // Keep a5pnr's registers where they are: no retiming, no duplication.
    writeln!(q, "set_global_assignment -name ALLOW_REGISTER_RETIMING OFF")?;
    for port in &nl.ports {
        let Some((ball, std)) = pins.get(&port.name) else { continue };
        let std = match std.as_str() {
            "3.3" => "3.3-V LVCMOS",
            "1.1" => "1.1-V",
            "1.2" => "1.2-V",
            s => bail!("quartus export: I/O standard {s}"),
        };
        writeln!(q, "set_location_assignment PIN_{ball} -to {}", port.name)?;
        writeln!(q, "set_instance_assignment -name IO_STANDARD \"{std}\" -to {}", port.name)?;
    }
    q.push_str(&fast);
    q.push_str(&loc);
    std::fs::write(format!("{dir}/{top}.qsf"), q)?;
    std::fs::write(format!("{dir}/{top}.qpf"), format!("PROJECT_REVISION = \"{top}\"\n"))?;
    // Over-constrained on purpose: quartus_sta's Fmax summary reports what
    // the placement reaches.
    let mut sdc = String::new();
    for port in &nl.ports {
        if port.input && matches!(port.net, Sig::Net(n) if p.clocks.contains(&n)) {
            writeln!(sdc, "create_clock -name {0} -period 2.000 [get_ports {{{0}}}]", port.name)?;
        }
    }
    std::fs::write(format!("{dir}/{top}.sdc"), sdc)?;
    Ok(())
}

/// A registered multiply of two W-bit operands (yosys/cells_sim.v A5_MUL).
const MUL_RTL: &str = r#"
module a5_mul #(parameter W = 18, parameter PW = 36, parameter SIGNED = 0) (
    input CLK, input [W-1:0] A, input [W-1:0] B, output reg [PW-1:0] P = 0
);
    wire [PW-1:0] prod = SIGNED ? $signed(A) * $signed(B) : A * B;
    always @(posedge CLK) P <= prod;
endmodule
"#;

/// yosys/cells_sim.v's A5_M20K, as RTL Quartus infers an M20K from (a
/// registered read address, the data unregistered unless OUTREG).
const M20K_RTL: &str = r#"
module a5_m20k #(parameter [16383:0] INIT = 16384'h0, parameter OUTREG = 0) (
    input CLK, input WE, input [3:0] BE, input RE,
    input [8:0] WADDR, input [31:0] WDATA, input [8:0] RADDR,
    output [31:0] RDATA
);
    (* ramstyle = "M20K, no_rw_check" *) reg [31:0] mem [0:511];
    reg [8:0] ra = 0;
    reg [31:0] q = 0;
    integer i;
    initial for (i = 0; i < 512; i = i + 1) mem[i] = INIT[32 * i +: 32];
    always @(posedge CLK) begin
        if (WE) begin
            if (BE[0]) mem[WADDR][7:0] <= WDATA[7:0];
            if (BE[1]) mem[WADDR][15:8] <= WDATA[15:8];
            if (BE[2]) mem[WADDR][23:16] <= WDATA[23:16];
            if (BE[3]) mem[WADDR][31:24] <= WDATA[31:24];
        end
        if (RE) ra <= RADDR;
        q <= mem[ra];
    end
    assign RDATA = OUTREG ? q : mem[ra];
endmodule
"#;
