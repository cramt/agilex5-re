// Board probe (2026-11-06): MLAB write data hold. The address, c[9:5], holds
// for 32 cycles; the write enable is c[0]; the data, c[3:0], changes at the
// edge that ends each write. In each following cycle (c[0] = 0) the word must
// hold the data of the cycle before. LED0 latches 1 (dark) at the first word
// that doesn't, from cycle 64 on; LED1 = armed, LED[7:2] = c[26:21].
module mwd (input wire CLOCK1_50, output reg [7:0] LED = 0);
    wire clk = CLOCK1_50;
    reg [26:0] c = 0;
    always @(posedge clk) c <= c + 1'b1;
    wire [3:0] p = c[3:0] - 4'd1;
    reg [3:0] m [0:31];
    always @(posedge clk) if (c[0]) m[c[9:5]] <= c[3:0];
    reg arm = 0;
    always @(posedge clk) arm <= arm | c[6];
    always @(posedge clk) begin
        LED[7:1] <= {c[26:21], arm};
        if (arm && !c[0] && c[4:0] != 5'd0 && m[c[9:5]] != p) LED[0] <= 1'b1;
    end
endmodule
