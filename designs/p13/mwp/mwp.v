// Board probe (2026-11-06): an MLAB's power-up contents. A 32 x 4 RAM with
// no initial contents (all 0) is read at a = c[27:23] (0.17 s per word);
// nothing is written for the first 10.7 s (c[29] = 0). LED[3:0] = m[a],
// LED[7:4] = a[3:0]: every word should read 0.
module mwp (input wire CLOCK1_50, output reg [7:0] LED = 0);
    wire clk = CLOCK1_50;
    reg [29:0] c = 0;
    always @(posedge clk) c <= c + 1'b1;
    wire [4:0] a = c[27:23];
    reg [3:0] m [0:31];
    always @(posedge clk) if (c[29] && c[0]) m[c[4:0]] <= c[8:5];
    always @(posedge clk) LED <= {a[3:0], m[a]};
endmodule
