// Phase 5 core features, development design (README (q)): M20K blocks (a
// simple dual-port RAM and an initialised ROM) and DSP blocks (a registered
// 18x18 signed multiply and a 27x27 multiply-accumulate). Data from an LFSR;
// each block is XOR-reduced to an output pin.
module mm (
    input  wire CLOCK1_50,
    output wire O0, O1, O2, O3
);
    wire clk = CLOCK1_50;
    reg [31:0] lfsr = 32'h1;
    always @(posedge clk) lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};
    reg [9:0] a = 0;
    always @(posedge clk) a <= a + 1'b1;

    // 512 x 32 simple dual-port RAM.
    (* ramstyle = "M20K, no_rw_check" *) reg [31:0] ram [0:511];
    reg [31:0] q;
    always @(posedge clk) begin
        ram[lfsr[8:0]] <= lfsr;
        q <= ram[a[8:0]];
    end

    // 1024 x 16 ROM with contents.
    (* romstyle = "M20K" *) reg [15:0] rom [0:1023];
    integer k;
    initial for (k = 0; k < 1024; k = k + 1) rom[k] = (k * 16'h9e37) ^ (k >> 3);
    reg [15:0] r;
    always @(posedge clk) r <= rom[a];

    // 18x18 signed multiply, registered in and out.
    reg signed [17:0] ma, mb; reg signed [35:0] mp;
    always @(posedge clk) begin ma <= lfsr[17:0]; mb <= lfsr[31:14]; mp <= ma * mb; end

    // 27x27 multiply-accumulate.
    reg [26:0] xa, xb; reg [63:0] acc;
    always @(posedge clk) begin xa <= lfsr[26:0]; xb <= {lfsr[4:0], lfsr[31:10]}; acc <= acc + xa * xb; end

    assign O0 = ^q;
    assign O1 = ^r;
    assign O2 = ^mp;
    assign O3 = ^acc;
endmodule
