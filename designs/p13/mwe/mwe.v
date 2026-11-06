// Board probe (2026-11-06): an MLAB write enable that changes every cycle, as
// a CPU's write-back pulse does. Word c[4:0] of a 32 x 4 RAM is written with
// {c[8:6], ~c[9]} (never 0 for long) only while c[0] = 1, so only the odd words are ever written and
// the even ones stay 0. From cycle 64 on, LED0 latches 1 (dark) at the first
// odd word read one cycle after its write that isn't what was written, LED1
// at the first even word that isn't 0. LED[7:3] = c[26:22], LED2 = armed.
module mwe (input wire CLOCK1_50, output reg [7:0] LED = 0);
    wire clk = CLOCK1_50;
    reg [26:0] c = 0;
    wire [9:0] c1 = c[9:0] - 10'd1;
    always @(posedge clk) c <= c + 1'b1;
    reg [3:0] m [0:31];
    always @(posedge clk) if (c[0]) m[c[4:0]] <= {c[8:6], ~c[9]};
    wire [3:0] q = m[c1[4:0]];
    reg arm = 0;
    always @(posedge clk) arm <= arm | c[6];
    always @(posedge clk) begin
        LED[7:2] <= {c[26:23], c[22], arm};
        if (arm && c1[0] && q != {c1[8:6], ~c1[9]}) LED[0] <= 1'b1;
        if (arm && !c1[0] && q != 4'd0) LED[1] <= 1'b1;
    end
endmodule
