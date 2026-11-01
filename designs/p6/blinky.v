// Phase 6 open flow, design 1 (README (w)): a 25-bit counter on CLOCK1_50,
// its top bit on LED0 (about 1.5 Hz).
module blinky (input wire CLOCK1_50, output wire LED0);
    reg [24:0] c = 0;
    always @(posedge CLOCK1_50) c <= c + 1'b1;
    assign LED0 = c[24];
endmodule
