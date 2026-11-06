// Phase 7's CPU milestone (README (z2)): Bruno Levy's FemtoRV32 Quark
// (RV32I; femtorv32_quark.v unmodified, BSD-3-Clause, LICENSE.femtorv32)
// running prog.vh (asm.py) out of an M20K. The program counts in x2 and
// stores the count to the LEDs: with SW0 down it waits 2^21 turns of a
// 6-cycle loop per count (LED0 toggles every 0.25 s at 50 MHz), with SW0 up
// 3 turns (fast enough to simulate). KEY0 (active low) resets.
//   0x000000-0x0007ff  program, M20K (read with the CPU's read strobe)
//   0x400000           store: LED[7:0] = data[7:0]
//   0x400004           load: {31'b0, SW0}
// femtorv32_quark.v's BENCH block zeroes x0, the cycle counter and the shift
// count at power-up, as the flops do on the device; it must be read after
// this file.
`define BENCH
module cpus (
    input  wire CLOCK1_50,
    input  wire KEY0, SW0,
    output wire [7:0] LED
);
`include "prog.vh"
    // Board probe (2026-11-06): the same CPU on a 25 MHz PLL clock.
    wire clk;
    A5_IOPLL #(.REF_KHZ(50000), .OUT_KHZ(25000)) p (.REFCLK(CLOCK1_50), .OUTCLK(clk));
    reg [3:0] rst_cnt = 0;
    wire resetn = &rst_cnt;
    always @(posedge clk)
        if (!KEY0) rst_cnt <= 0;
        else if (!resetn) rst_cnt <= rst_cnt + 1'b1;

    wire [31:0] mem_addr, mem_wdata, rom_q;
    wire [3:0] mem_wmask;
    wire mem_rstrb;
    reg io_sel = 0;
    reg [31:0] io_q = 0;
    FemtoRV32 core (
        .clk(clk), .mem_addr(mem_addr), .mem_wdata(mem_wdata), .mem_wmask(mem_wmask),
        .mem_rdata(io_sel ? io_q : rom_q), .mem_rstrb(mem_rstrb), .mem_rbusy(1'b0), .mem_wbusy(1'b0),
        .reset(resetn));

    A5_M20K #(.INIT(PROG)) rom (
        .CLK(clk), .WE(1'b0), .BE(4'b1111), .RE(mem_rstrb),
        .WADDR(9'd0), .WDATA(32'd0), .RADDR(mem_addr[10:2]), .RDATA(rom_q));

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
