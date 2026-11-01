// Simulation models of a5pnr's cells (README (w)), used on both sides of
// tools/a5pnr/simcheck.py: the source design's instances and the netlist
// rebuilt from the image. A5_M20K: write and read addresses registered on
// CLK, written every cycle, read data from the registered read address;
// contents start at zero (the idle image's M20K contents).
module A5_M20K (
    input CLK,
    input WE,
    input [8:0] WADDR,
    input [31:0] WDATA,
    input [8:0] RADDR,
    output [31:0] RDATA
);
    reg [31:0] mem [0:511];
    reg [8:0] ra = 0;
    integer i;
    initial for (i = 0; i < 512; i = i + 1) mem[i] = 0;
    always @(posedge CLK) begin
        if (WE) mem[WADDR] <= WDATA;
        ra <= RADDR;
    end
    assign RDATA = mem[ra];
endmodule
