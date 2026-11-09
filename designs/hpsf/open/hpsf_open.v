// The fabric half of designs/hpsf for the open flow (yosys, a5pnr, a5db):
// two FPGA-to-HPS interrupt lines that the HPS reads in GICD_ISPENDR1,
// different from Quartus' build so a load is told apart (README, "HPS-first").
//   irq0  f2h_irq0[0], bit 17: cnt[23], 2.98 Hz (Quartus' build: cnt[25])
//   irq3  f2h_irq0[3], bit 20: cnt[24] & cnt[23], high a quarter of the time
// The HPS's other inputs are the RPI ties kept from Quartus' build (irq0[2] = 0;
// undriven ones, irq0[1], [4], [5] among them, read 1)
// (tools/hps_shell.sh).
module hpsf_open (
    input  wire CLOCK1_50,
    output wire irq0,
    output wire irq3
);
   reg [24:0] cnt = 25'd0;
   always @(posedge CLOCK1_50) cnt <= cnt + 1'b1;
   assign irq0 = cnt[23];
   assign irq3 = cnt[24] & cnt[23];
endmodule
