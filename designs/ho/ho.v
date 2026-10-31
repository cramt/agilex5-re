// Held-out check of the idle model (README (n)): a design none of the idle
// derivation's images had. A 23-bit counter from CLOCK1_50 on LED1 (DJ32),
// another IO96 output than blinky's LED0, so its pin bits test whether
// db/sm4revb/io/DF35_out.tsv carries over to the next pin of the lane.
module ho (
    input  wire CLOCK1_50,
    output wire LED1
);
    reg [22:0] c = 0;
    always @(posedge CLOCK1_50) c <= c + 1'b1;
    assign LED1 = c[22];
endmodule
