// Board probe (2026-11-06): MLAB read-after-write timing at 50 MHz. Every
// cycle word c[4:0] of a 32 x 4 RAM is written with c[8:5]; four reads look
// at the words written 0 (the one being written: the old word), 1, 2 and 3
// cycles before. LED[k] latches 1 (dark) at the first read of lag k whose
// value isn't what the RTL gives, from cycle 64 on; LED[7:4] = c[26:23].
module mwt (input wire CLOCK1_50, output reg [7:0] LED = 0);
    wire clk = CLOCK1_50;
    reg [26:0] c = 0;
    always @(posedge clk) c <= c + 1'b1;
    // A flop as the write enable: a5pnr refuses a constant one.
    reg we = 0;
    always @(posedge clk) we <= 1'b1;
    reg [3:0] m [0:31];
    always @(posedge clk) if (we) m[c[4:0]] <= c[8:5];
    wire [8:0] c1 = c[8:0] - 9'd1, c2 = c[8:0] - 9'd2, c3 = c[8:0] - 9'd3, c0 = c[8:0] - 9'd32;
    reg arm = 0;
    always @(posedge clk) arm <= arm | c[6];
    always @(posedge clk) begin
        LED[7:4] <= c[26:23];
        if (arm && m[c[4:0]] != c0[8:5]) LED[0] <= 1'b1;
        if (arm && m[c1[4:0]] != c1[8:5]) LED[1] <= 1'b1;
        if (arm && m[c2[4:0]] != c2[8:5]) LED[2] <= 1'b1;
        if (arm && m[c3[4:0]] != c3[8:5]) LED[3] <= 1'b1;
    end
endmodule
