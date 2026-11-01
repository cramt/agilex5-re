// Phase 8 oracle design (README (z5)): MLAB RAM as Quartus Pro builds it.
// Two RAMs forced into MLABs, written and read from an LFSR so every
// address, data and enable pin carries its own signal: A is 32 x 20 with an
// asynchronous read (registered in the fabric), B is 16 x 8 with its read
// data registered. Outputs XOR-reduce what was read.
module ml (
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
    // Empty at power-up, as Quartus' image has it (all-zero masks); the
    // compiled run (specimens/runs/ml0) predates these two initial blocks.
    integer i;
    initial for (i = 0; i < 32; i = i + 1) ma[i] = 0;
    always @(posedge clk) if (lfsr[0]) ma[lfsr[5:1]] <= lfsr[25:6];
    wire [19:0] qa = ma[cnt[4:0]];
    reg [19:0] ra = 0;
    always @(posedge clk) ra <= qa;

    (* ramstyle = "MLAB, no_rw_check" *) reg [7:0] mb [0:15];
    initial for (i = 0; i < 16; i = i + 1) mb[i] = 0;
    always @(posedge clk) if (KEY0 & lfsr[3]) mb[lfsr[9:6]] <= lfsr[31:24];
    reg [7:0] rb = 0;
    always @(posedge clk) rb <= mb[cnt[8:5]];

    assign O = {^rb[7:4], ^rb[3:0], ^ra[19:10], ^ra[9:0]};
endmodule
