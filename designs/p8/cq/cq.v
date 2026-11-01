// Phase 8's Quartus reference (README (z3)): designs/p7/cpu's CPU with the
// program ROM written as an inferred M20K, so Quartus Pro fits the same RTL
// a5pnr does and its ALM count and Fmax can be compared. The ROM behaves as
// A5_M20K does there: the read address is registered when the CPU's read
// strobe is high, and the data comes out unregistered after it.
`define BENCH
module cq (
    input  wire CLOCK1_50,
    input  wire KEY0, SW0,
    output wire [7:0] LED
);
`include "prog.vh"
    wire clk = CLOCK1_50;
    reg [3:0] rst_cnt = 0;
    wire resetn = &rst_cnt;
    always @(posedge clk)
        if (!KEY0) rst_cnt <= 0;
        else if (!resetn) rst_cnt <= rst_cnt + 1'b1;

    wire [31:0] mem_addr, mem_wdata;
    wire [3:0] mem_wmask;
    wire mem_rstrb;
    reg io_sel = 0;
    reg [31:0] io_q = 0;

    (* ramstyle = "M20K, no_rw_check" *) reg [31:0] rom [0:511];
    integer k;
    initial for (k = 0; k < 512; k = k + 1) rom[k] = PROG[32 * k +: 32];
    reg [31:0] rom_q;
    always @(posedge clk) if (mem_rstrb) rom_q <= rom[mem_addr[10:2]];

    FemtoRV32 core (
        .clk(clk), .mem_addr(mem_addr), .mem_wdata(mem_wdata), .mem_wmask(mem_wmask),
        .mem_rdata(io_sel ? io_q : rom_q), .mem_rstrb(mem_rstrb), .mem_rbusy(1'b0), .mem_wbusy(1'b0),
        .reset(resetn));

    reg [7:0] leds = 0;
    always @(posedge clk) begin
        if (mem_rstrb) begin
            io_sel <= mem_addr[22];
            io_q <= {31'b0, SW0};
        end
        if (|mem_wmask && mem_addr[22]) leds <= mem_wdata[7:0];
    end
    assign LED = leds;
endmodule
