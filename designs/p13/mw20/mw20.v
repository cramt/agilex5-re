// Board probe (2026-11-06): a full MLAB (32 x 20, ten ALMs, both halves),
// written as mw does. LED[k] (k = 0..4) is 1 while nibble k of the word read
// equals its expected value (all five once every word has been written,
// 5.4 s); LED[7:5] = a[2:0] as a heartbeat.
module mw20 (input wire CLOCK1_50, output reg [7:0] LED = 0);
    wire clk = CLOCK1_50;
    reg [27:0] c = 0;
    always @(posedge clk) c <= c + 1'b1;
    wire [4:0] a = c[27:23];
    wire [4:0] w = a + 1'b1;
    function [3:0] f(input [4:0] x, input [3:0] k); f = x[3:0] * 4'd5 + 4'd3 + {x[4], 3'b0} ^ (k * 4'd3); endfunction
    function [19:0] word(input [4:0] x); word = {f(x, 4), f(x, 3), f(x, 2), f(x, 1), f(x, 0)}; endfunction
    reg [19:0] m [0:31];
    always @(posedge clk) if (c[22]) m[w] <= word(w);
    wire [19:0] q = m[a], e = word(a);
    integer k;
    always @(posedge clk) begin
        LED[7:5] <= a[2:0];
        for (k = 0; k < 5; k = k + 1) LED[k] <= q[4*k +: 4] == e[4*k +: 4];
    end
endmodule
