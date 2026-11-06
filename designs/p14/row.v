// The camera's reference (README, "Self-tests"): verdict.v with every round
// good (row blinks at 1 Hz, PASS) or, with MUTATE = 1, every round bad (row
// steadily lit). The detector is checked on these before any self-test.
module row (input wire CLOCK1_50, output wire [7:0] LED);
    localparam MUTATE = 0;   // build.sh <name>_mut sets 1: every round bad
    verdict vd (.clk(CLOCK1_50), .round(1'b1), .bad(MUTATE != 0), .LED(LED));
endmodule
