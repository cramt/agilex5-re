// Phase 7 open flow (README (y)): a PLL clock. CLOCK1_50 into V16's IOPLL
// for 100 MHz, which clocks a 27-bit counter; a second counter runs on
// CLOCK2_50 (V16 feeds the PLL only, as in Quartus' pl).
//   LED[3:0]  the 100 MHz counter's bits 26:23 (LED3 toggles every 0.67 s);
//   LED[7:4]  the CLOCK2_50 counter's bits 25:22 (LED7 toggles every 0.67 s).
// With the PLL at 100 MHz, LED3 and LED7 toggle at the same rate.
module pll (
    input  wire CLOCK1_50, CLOCK2_50,
    output wire [7:0] LED
);
    wire clk100;
    A5_IOPLL #(.REF_KHZ(50000), .OUT_KHZ(100000)) p (.REFCLK(CLOCK1_50), .OUTCLK(clk100));
    reg [26:0] a = 0;
    always @(posedge clk100) a <= a + 1'b1;
    reg [25:0] b = 0;
    always @(posedge CLOCK2_50) b <= b + 1'b1;
    assign LED = {b[25:22], a[26:23]};
endmodule
