// Simulation models of a5pnr's cells (README (w), (y)), used on both sides
// of tools/a5pnr/simcheck.py: the source design's instances and the netlist
// rebuilt from the image. A5_M20K: write address, data and enables are
// registered on CLK; byte b of word WADDR is written when WE and BE[b]; when
// RE is high the word at RADDR is read (before that edge's write) and held
// while RE is low, through one more register when OUTREG [obs: Quartus' mq
// image against its RTL, README (z6); phases 6-8 registered only the
// address, so a later write to it showed through]. Contents start at INIT
// (bit 32a + k is word a, bit k).
module A5_M20K #(parameter [16383:0] INIT = 16384'h0, parameter OUTREG = 0) (
    input CLK,
    input WE,
    input [3:0] BE,
    input RE,
    input [8:0] WADDR,
    input [31:0] WDATA,
    input [8:0] RADDR,
    output [31:0] RDATA
);
    reg [31:0] mem [0:511];
    reg [31:0] r = 0, q = 0;
    integer i;
    initial for (i = 0; i < 512; i = i + 1) mem[i] = INIT[32 * i +: 32];
    always @(posedge CLK) begin
        if (WE) begin
            if (BE[0]) mem[WADDR][7:0] <= WDATA[7:0];
            if (BE[1]) mem[WADDR][15:8] <= WDATA[15:8];
            if (BE[2]) mem[WADDR][23:16] <= WDATA[23:16];
            if (BE[3]) mem[WADDR][31:24] <= WDATA[31:24];
        end
        if (RE) r <= mem[RADDR];
        q <= r;
    end
    assign RDATA = OUTREG ? q : r;
endmodule

// An M20K as the image holds it, for the readback (src/readback.rs): the
// physical 40-bit word in four 10-bit byte lanes (BE[b] writes bits
// 10b..10b + 9), as A5_M20K otherwise. Which 32 of the 40 bits a design
// uses is the fitter's choice: a5pnr takes 8 of each lane, Quartus without
// byte enables bits 0..31 (mq, README (z6)).
module A5_M20K40 #(parameter [20479:0] INIT = 20480'h0, parameter OUTREG = 0) (
    input CLK,
    input WE,
    input [3:0] BE,
    input RE,
    input [8:0] WADDR,
    input [39:0] WDATA,
    input [8:0] RADDR,
    output [39:0] RDATA
);
    reg [39:0] mem [0:511];
    reg [39:0] r = 0, q = 0;
    integer i;
    initial for (i = 0; i < 512; i = i + 1) mem[i] = INIT[40 * i +: 40];
    always @(posedge CLK) begin
        if (WE) for (i = 0; i < 4; i = i + 1) if (BE[i]) mem[WADDR][10 * i +: 10] <= WDATA[10 * i +: 10];
        if (RE) r <= mem[RADDR];
        q <= r;
    end
    assign RDATA = OUTREG ? q : r;
endmodule

// A5_IOPLL as simcheck sees it: the output follows the reference. The
// check is of logic, and every clock ticks together there anyway.
module A5_IOPLL #(parameter REF_KHZ = 50000, parameter OUT_KHZ = 100000) (
    input REFCLK,
    output OUTCLK
);
    assign OUTCLK = REFCLK;
endmodule

// A5_ARITH as tennm_lcell_comb's arithmetic mode (yosys/cells.v), for
// simulating yosys' own netlist (simcheck's netlist mode).
module A5_ARITH #(parameter [15:0] P = 16'h0, parameter [15:0] G = 16'h0) (
    input A, B, C, D, CI,
    output SO, CO
);
    wire p = P[{D, C, B, A}], g = G[{D, C, B, A}];
    assign SO = CI ^ p;
    assign CO = p ? CI : g;
endmodule
