// Board probe (2026-11-06): every LED from a plain output register, no enable
// or clear, to tell which IO96 pins' output registers work on silicon.
// LED[7:4] = cnt[25:22] (0.75, 1.49, 2.98, 5.96 Hz), LED[3:0] the same bits inverted.
module iol (input wire CLOCK1_50, output wire [7:0] LED);
    reg [25:0] cnt = 0;
    always @(posedge CLOCK1_50) cnt <= cnt + 1'b1;
    reg [7:0] led = 0;
    always @(posedge CLOCK1_50) led <= {cnt[22], cnt[23], cnt[24], cnt[25], ~cnt[22], ~cnt[23], ~cnt[24], ~cnt[25]};
    assign LED = led;
endmodule
