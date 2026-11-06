// Board probe (2026-11-06): does an MLAB write leak into the word on its read
// address? For the first 1.34 s (c[26] = 0) only words W = {c[26], c[6:3]}
// are written, for one cycle in eight (c[2:0] = 3), and W doesn't change
// around the write; the read address is W with bit 4 flipped, a word nothing
// has written yet. (a5pnr wants no address bit constant on one port only.)
// LED0 latches 1 (dark) the first time that word reads nonzero, from cycle
// 256 until c[26] rises; LED1 = armed, LED[7:2] = c[26:21].
module mwr (input wire CLOCK1_50, output reg [7:0] LED = 0);
    wire clk = CLOCK1_50;
    reg [26:0] c = 0;
    always @(posedge clk) c <= c + 1'b1;
    wire [4:0] w = {c[26], c[6:3]};
    reg [3:0] m [0:31];
    always @(posedge clk) if (c[2:0] == 3'd3) m[w] <= {c[9:7], ~c[10]};
    reg arm = 0;
    always @(posedge clk) arm <= arm | c[8];
    always @(posedge clk) begin
        LED[7:1] <= {c[26:21], arm};
        if (arm && !c[26] && m[w ^ 5'h10] != 4'd0) LED[0] <= 1'b1;
    end
endmodule
