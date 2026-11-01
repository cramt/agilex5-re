// Phase 7 oracle design (README (y)): the LAB's register controls, each
// from its own pin so a decoded image shows which LAB input carries which
// signal: two clock enables, a synchronous clear, an active-high and an
// active-low asynchronous clear, both priorities of enable and clear; a
// second clock domain; carry chains with a pin as carry-in and with a
// constant carry-in (subtract); an input and an output register in the I/O
// cell. Data from an LFSR; each group is XOR-reduced to an output.
module ct (
    input  wire CLOCK1_50,
    input  wire CLOCK2_50,
    input  wire CEA, CEB, SCLR, ACLR, ACLRN, CIN, DIN,
    output wire [7:0] O,
    output wire DOUT
);
    wire clk = CLOCK1_50;
    reg [23:0] lfsr = 24'h1;
    always @(posedge clk) lfsr <= {lfsr[22:0], lfsr[23] ^ lfsr[22] ^ lfsr[21] ^ lfsr[16]};

    reg [7:0] r1, r2, r3, r4, r5, r7, r8;
    always @(posedge clk) if (CEA) r1 <= lfsr[7:0];
    always @(posedge clk) if (CEB) r2 <= lfsr[15:8];
    always @(posedge clk) if (SCLR) r3 <= 0; else r3 <= lfsr[23:16];
    always @(posedge clk or posedge ACLR) if (ACLR) r4 <= 0; else r4 <= lfsr[7:0] ^ lfsr[15:8];
    always @(posedge clk or negedge ACLRN) if (!ACLRN) r5 <= 0; else r5 <= lfsr[15:8] ^ lfsr[23:16];
    always @(posedge clk) if (CEA) begin if (SCLR) r7 <= 0; else r7 <= lfsr[11:4]; end
    always @(posedge clk) if (SCLR) r8 <= 0; else if (CEB) r8 <= lfsr[19:12];

    reg [8:0] s, t;
    always @(posedge clk) begin
        s <= lfsr[7:0] + lfsr[15:8] + CIN;
        t <= lfsr[7:0] - lfsr[23:16];
    end

    reg [7:0] c2;
    always @(posedge CLOCK2_50) c2 <= c2 + 1'b1;

    reg din_r, dout_r;
    always @(posedge clk) begin
        din_r <= DIN;
        dout_r <= ^r1 ^ din_r;
    end

    assign O[0] = ^r1;
    assign O[1] = ^r2;
    assign O[2] = ^r3;
    assign O[3] = ^r4;
    assign O[4] = ^r5;
    assign O[5] = ^r7 ^ ^r8;
    assign O[6] = ^s ^ ^t;
    assign O[7] = ^c2;
    assign DOUT = dout_r;
endmodule
