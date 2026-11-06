// Self-test verdict on the DE25-Nano's whole LED row (README, "Self-tests").
// The webcam sees the eight LEDs as one smeared blob, so every LED shows the
// same thing (active low: 0 lights):
//   PASS  the row blinks at 1 Hz, 50% duty (0.5 s lit, 0.5 s dark)
//   FAIL  the row is steadily lit
// `round` pulses once per completed round that matched; `bad` is a round's
// mismatch. FAIL latches on any mismatch, and on a half second without a
// good round (the watchdog: a hung test must not keep blinking). Every test
// here finishes a round in well under a millisecond.
module verdict (
    input  wire clk,
    input  wire round,
    input  wire bad,
    output reg  [7:0] LED = 8'hff
);
    localparam HALF = 25_000_000;     // 0.5 s of CLOCK1_50
    reg [24:0] t = 0;
    reg ph = 0, seen = 0, fail = 0;
    always @(posedge clk) begin
        t <= t == HALF - 1 ? 25'd0 : t + 1'b1;
        if (t == HALF - 1) ph <= ~ph;
        if (bad || (t == HALF - 1 && !seen && !round)) fail <= 1'b1;
        seen <= t == HALF - 1 ? 1'b0 : seen | round;
        LED <= fail ? 8'h00 : {8{ph}};
    end
endmodule
