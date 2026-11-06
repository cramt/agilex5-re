// M20K self-test (README, "Self-tests"): ramx's write and read paths with no
// switch. Each round, with p the round counter:
//   0  write pat(p, a) to all 512 words (every byte enable)
//   1  write ~pat(p, a) through byte enable p mod 4 alone, every word
//   2  read every word (RE, output register: two cycles' latency) and
//      compare with pat(p, a), byte p mod 4 inverted
//   3  drain the read pipeline; a round with no mismatch pulses `round`
// Any mismatch latches FAIL (verdict.v: row steadily lit); good rounds keep
// the row blinking at 1 Hz. A round is 1,540 cycles (31 us).
module ramx_st (input wire CLOCK1_50, output wire [7:0] LED);
    localparam MUTATE = 0;   // build.sh ramx_st_mut sets 1: word 137's expected bit 5 wrong
    wire clk = CLOCK1_50;
    // Each bit depends on the round and the address, and every bit toggles
    // over the rounds, so stuck bits, swapped addresses and stale words all
    // show up.
    function [31:0] pat(input [15:0] p, input [8:0] a);
        pat = {p ^ {a[7:0], 7'b0, a[8]}, ~p ^ {7'b0, a}};
    endfunction
    reg [1:0] ph = 0;
    reg [8:0] a = 0;
    reg [15:0] p = 0;
    reg [1:0] dr = 0;
    wire [3:0] lane = 4'b0001 << p[1:0];
    wire [31:0] lanebits = {{8{lane[3]}}, {8{lane[2]}}, {8{lane[1]}}, {8{lane[0]}}};

    reg v1 = 0, v2 = 0;
    reg [8:0] a1 = 0, a2 = 0;
    wire [31:0] q;
    wire [31:0] e = pat(p, a2) ^ lanebits ^ (MUTATE != 0 && a2 == 9'd137 ? 32'h20 : 32'h0);
    A5_M20K #(.OUTREG(1)) m (
        .CLK(clk), .WE(ph == 2'd0 || ph == 2'd1), .BE(ph == 2'd0 ? 4'b1111 : lane), .RE(ph == 2'd2),
        .WADDR(a), .WDATA(ph == 2'd0 ? pat(p, a) : ~pat(p, a)), .RADDR(a), .RDATA(q));

    reg err = 0, round = 0, bad = 0;
    always @(posedge clk) begin
        round <= 1'b0; bad <= 1'b0;
        v1 <= ph == 2'd2; a1 <= a; v2 <= v1; a2 <= a1;
        if (v2 && q != e) err <= 1'b1;
        if (ph != 2'd3) begin
            a <= a + 1'b1;
            if (a == 9'd511) ph <= ph + 1'b1;
        end else begin
            dr <= dr + 1'b1;
            if (dr == 2'd3) begin
                round <= !err; bad <= err; err <= 1'b0;
                p <= p + 1'b1; ph <= 2'd0;
            end
        end
    end
    verdict vd (.clk(clk), .round(round), .bad(bad), .LED(LED));
endmodule
