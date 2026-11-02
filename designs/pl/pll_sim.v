// For simulation only (tools/alm/oracle.sh, README (z6)): the IP prep.tcl
// generates, as simcheck sees a PLL (output follows the reference, every
// clock ticks together). `locked` is x: the readback has no model of the
// PLL's lock output (its LED1 is x too), so the oracle compares LED0 and
// the registers only.
module pll (input wire refclk, input wire rst, output wire outclk_0, output wire locked);
    assign outclk_0 = refclk;
    assign locked = 1'bx;
endmodule
