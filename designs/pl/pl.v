// Phase 5 IOPLL (README (r)): CLOCK1_50 into an IOPLL (prep.tcl), its 100 MHz
// output clocks a 24-bit counter whose top bit drives LED0. KEY0 resets the
// PLL; LED1 shows locked.
module pl (
    input  wire CLOCK1_50,
    input  wire KEY0,
    output wire LED0,
    output wire LED1
);
    wire clk, locked;
    pll u_pll (.refclk(CLOCK1_50), .rst(~KEY0), .outclk_0(clk), .locked(locked));
    reg [23:0] c = 0;
    always @(posedge clk) c <= c + 1'b1;
    assign LED0 = c[23];
    assign LED1 = locked;
endmodule
