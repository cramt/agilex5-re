// Phase 5 core features, development design (README (q)): carry chains and
// arithmetic, a ternary (shared-arithmetic) adder, register controls
// (asynchronous clear, synchronous clear, clock enable, synchronous load),
// register-only cells and an MLAB used as RAM. Data comes from an LFSR so
// nothing folds away; each block is XOR-reduced to an output pin. The
// initial 0s are the device's power-up state, for the oracle (README (z6));
// the compiled run (specimens/runs/ar0) predates them.
module ar (
    input  wire CLOCK1_50,
    input  wire KEY0,      // asynchronous clear, active low
    input  wire KEY1,      // synchronous clear, active low
    input  wire EN,        // clock enable
    output wire O0, O1, O2, O3, O4
);
    wire clk = CLOCK1_50, aclr = ~KEY0, sclr = ~KEY1;
    reg [31:0] lfsr = 32'h1;
    always @(posedge clk) lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};

    // 32-bit accumulator: carry chain, enable, synchronous clear.
    reg [31:0] acc = 0;
    always @(posedge clk) if (sclr) acc <= 0; else if (EN) acc <= acc + lfsr;

    // Ternary adder: shared arithmetic mode.
    reg [15:0] t = 0;
    always @(posedge clk) t <= lfsr[15:0] + lfsr[31:16] + acc[15:0];

    // Down counter with asynchronous clear and synchronous load.
    reg [11:0] dc = 0;
    always @(posedge clk or posedge aclr)
        if (aclr) dc <= 0;
        else if (lfsr[3]) dc <= lfsr[27:16];
        else dc <= dc - 1'b1;

    // Comparator and subtractor.
    reg lt = 0; reg [15:0] d = 0;
    always @(posedge clk) begin lt <= acc[31:16] < t; d <= t - acc[15:0]; end

    // Register-only cells: a shift register (no LUT between registers).
    reg [7:0] sr = 0;
    always @(posedge clk) sr <= {sr[6:0], lfsr[5]};

    // 32 x 16 RAM in an MLAB.
    (* ramstyle = "MLAB, no_rw_check" *) reg [15:0] mem [0:31];
    reg [15:0] q = 0;
    integer i;
    initial for (i = 0; i < 32; i = i + 1) mem[i] = 0;
    always @(posedge clk) begin
        if (lfsr[7]) mem[lfsr[12:8]] <= lfsr[31:16];
        q <= mem[acc[4:0]];
    end

    assign O0 = ^acc;
    assign O1 = ^t ^ lt;
    assign O2 = ^dc;
    assign O3 = ^d ^ sr[7];
    assign O4 = ^q;
endmodule
