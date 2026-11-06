// RTL check of a self-test (README, "Self-tests"): from power-up, counts the
// good rounds and reports whether FAIL latched (verdict.v's watchdog never
// fires in so few cycles):
//   designs/p14/rtl.sh <name> [cycles]
`timescale 1ns/1ps
module tb;
    reg clk = 0;
    wire [7:0] led;
    `TOP dut(.CLOCK1_50(clk), .LED(led));
    integer cyc = 0, rounds = 0, failed_at = -1;
    always #10 clk = ~clk;
    always @(posedge clk) begin
        cyc <= cyc + 1;
        if (dut.vd.round) rounds <= rounds + 1;
        if (dut.vd.bad && failed_at < 0) failed_at <= cyc;
        if (cyc == `CYCLES) begin
            if (failed_at < 0) $display("%0d cycles: %0d good rounds, no FAIL", cyc, rounds);
            else $display("%0d cycles: %0d good rounds, FAIL at cycle %0d", cyc, rounds, failed_at);
            $finish;
        end
    end
endmodule
