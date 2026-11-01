// Simulation models of a5pnr's cells (README (w), (y)), used on both sides
// of tools/a5pnr/simcheck.py: the source design's instances and the netlist
// rebuilt from the image. A5_M20K: write address, data and enables are
// registered on CLK; byte b of word WADDR is written when WE and BE[b]; the
// read address is registered when RE; read data is the word at the
// registered read address, through one more register when OUTREG. Contents
// start at INIT (bit 32a + k is word a, bit k).
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
