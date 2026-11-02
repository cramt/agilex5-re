// Phase 9 oracle design (README (z6)): MLAB RAM with initial contents, to
// find which mask bit holds which word. As designs/p8/ml, but every word
// starts at its own value: A is 32 x 20 with an asynchronous read, B is
// 16 x 8 with its read data registered. KEY0 high enables both writes; held
// low, the reads walk the initial contents.
module mi (
    input  wire CLOCK1_50,
    input  wire KEY0,
    output wire [3:0] O
);
    wire clk = CLOCK1_50;
    reg [31:0] lfsr = 32'h1;
    always @(posedge clk) lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};
    reg [15:0] cnt = 0;
    always @(posedge clk) cnt <= cnt + 1'b1;

    (* ramstyle = "MLAB, no_rw_check" *) reg [19:0] ma [0:31];
    (* ramstyle = "MLAB, no_rw_check" *) reg [7:0] mb [0:15];
    integer i;
    initial for (i = 0; i < 32; i = i + 1) ma[i] = (i * 20'h9e37 + 20'h1234) ^ (i << 15);
    initial for (i = 0; i < 16; i = i + 1) mb[i] = i * 8'h1d + 8'h5a;
    always @(posedge clk) if (KEY0 & lfsr[0]) ma[lfsr[5:1]] <= lfsr[25:6];
    wire [19:0] qa = ma[cnt[4:0]];
    reg [19:0] ra = 0;
    always @(posedge clk) ra <= qa;

    always @(posedge clk) if (KEY0 & lfsr[3]) mb[lfsr[9:6]] <= lfsr[31:24];
    reg [7:0] rb = 0;
    always @(posedge clk) rb <= mb[cnt[8:5]];

    assign O = {^rb[7:4], ^rb[3:0], ^ra[19:10], ^ra[9:0]};
endmodule
