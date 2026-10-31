// A second DCM P2C clock mux for the rule in README (n): CLOCK2_50 (BF23,
// hvio_1_1, bank 6E) forced onto a global clock, an 8-bit counter, its top
// bit on a pin of the same bank. Blinky, fz0 and rt0 all clock from V16.
module gc (
    input  wire CLOCK2_50,
    output wire OUT
);
    reg [7:0] c = 0;
    always @(posedge CLOCK2_50) c <= c + 1'b1;
    assign OUT = c[7];
endmodule
