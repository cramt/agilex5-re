// MLAB self-test (README, "Self-tests"): a 32 x 32 RAM shaped like
// FemtoRV32's register file (one write port, two registered read ports).
// First, once: read every word and expect its power-up 0 (an empty MLAB left
// at the idle mask reads all-ones, (z9)). Then each round, with p the round
// counter:
//   W  write pat(p, i) to word i, i = 0..31
//   R  read word i on port A and word 31 - i on port B, compare both
//   D  two cycles for the last compare; no mismatch pulses `round`
// Any mismatch latches FAIL (verdict.v: row steadily lit). A round is 66
// cycles.
module mlab_st (input wire CLOCK1_50, output wire [7:0] LED);
    localparam MUTATE = 0;   // build.sh mlab_st_mut sets 1: word 13's expected bit 9 wrong
    wire clk = CLOCK1_50;
    localparam W = 2'd0, R = 2'd1, Z = 2'd2, D = 2'd3;
    function [31:0] pat(input [15:0] p, input [4:0] i);
        pat = {p ^ {i, 3'b0, i, 3'b0}, ~p ^ {3'b0, i, 3'b0, i}};
    endfunction
    function [31:0] exp(input [15:0] p, input [4:0] i, input zero);
        exp = (zero ? 32'h0 : pat(p, i)) ^ (MUTATE != 0 && !zero && i == 5'd13 ? 32'h200 : 32'h0);
    endfunction

    reg [1:0] ph = Z;
    reg [4:0] i = 0;
    reg [15:0] p = 0;
    reg [31:0] rf [0:31];
    integer k;
    initial for (k = 0; k < 32; k = k + 1) rf[k] = 32'h0;
    reg [31:0] qa = 0, qb = 0;
    wire re = ph == R || ph == Z;
    always @(posedge clk) begin
        if (ph == W) rf[i] <= pat(p, i);
        if (re) begin qa <= rf[i]; qb <= rf[~i]; end
    end

    reg v = 0, zero = 0, d = 0, err = 0, round = 0, bad = 0;
    reg [4:0] ia = 0, ib = 0;
    always @(posedge clk) begin
        round <= 1'b0; bad <= 1'b0;
        v <= re; ia <= i; ib <= ~i; zero <= ph == Z;
        if (v && (qa != exp(p, ia, zero) || qb != exp(p, ib, zero))) err <= 1'b1;
        if (ph != D) begin
            i <= i + 1'b1;
            if (i == 5'd31) ph <= ph == W ? R : D;
        end else begin
            d <= ~d;
            if (d) begin
                round <= !err; bad <= err; err <= 1'b0;
                p <= p + 1'b1; ph <= W;
            end
        end
    end
    verdict vd (.clk(clk), .round(round), .bad(bad), .LED(LED));
endmodule
