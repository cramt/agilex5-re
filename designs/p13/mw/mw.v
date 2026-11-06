// Board probe (2026-11-06): MLAB RAM writes, with no switch or key. A 32 x 4
// RAM starts empty; every 2^23 cycles (0.17 s) the read address a steps, and
// while c[22] is high the next word, a + 1, is written with f(a + 1). Once
// every word has been written (5.4 s), LED[3:0] = m[a] equals LED[7:4] = f(a)
// in every step if the MLAB's write works.
module mw (input wire CLOCK1_50, output reg [7:0] LED = 0);
    wire clk = CLOCK1_50;
    reg [27:0] c = 0;
    always @(posedge clk) c <= c + 1'b1;
    wire [4:0] a = c[27:23];
    wire [4:0] w = a + 1'b1;
    function [3:0] f(input [4:0] x); f = x[3:0] * 4'd5 + 4'd3 + {x[4], 3'b0}; endfunction
    reg [3:0] m [0:31];
    always @(posedge clk) if (c[22]) m[w] <= f(w);
    always @(posedge clk) LED <= {f(a), m[a]};
endmodule
