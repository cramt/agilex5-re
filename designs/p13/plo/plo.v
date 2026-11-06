// Board probe (2026-11-06): pll's 100 MHz domain alone (no CLOCK2_50
// counter), LED[3:0] = a[26:23]: LED3 0.75 Hz, LED0 5.96 Hz if the PLL runs.
module plo (input wire CLOCK1_50, output wire [3:0] LED);
    wire clk100;
    A5_IOPLL #(.REF_KHZ(50000), .OUT_KHZ(100000)) p (.REFCLK(CLOCK1_50), .OUTCLK(clk100));
    reg [26:0] a = 0;
    always @(posedge clk100) a <= a + 1'b1;
    assign LED = a[26:23];
endmodule
