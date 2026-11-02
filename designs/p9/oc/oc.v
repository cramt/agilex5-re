// Phase 9 oracle design (README (z6)): carry chains and register controls
// as Quartus Pro builds them, retiming off. As designs/ct without its I/O
// registers, plus: a 64-bit accumulator (a chain over four LABs), a
// comparison whose result is a chain's carry out, a register preset by the
// asynchronous clear, registers that power up at 1, and enable and clear
// in both priorities. Each group is XOR-reduced to an output.
module oc (
    input  wire CLOCK1_50,
    input  wire CLOCK2_50,
    input  wire CEA, CEB, SCLR, ACLR, ACLRN, CIN, DIN,
    output wire [8:0] O
);
    wire clk = CLOCK1_50;
    reg [31:0] lfsr = 32'h1;
    always @(posedge clk) lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};

    reg [63:0] acc = 0;
    always @(posedge clk) if (CEA) acc <= acc + {lfsr, lfsr ^ {lfsr[15:0], lfsr[31:16]}};

    reg [7:0] r1 = 0, r3 = 0, r4 = 0, r5 = 0, r7 = 0, r8 = 0, pr = 0;
    always @(posedge clk) if (CEB) r1 <= lfsr[7:0];
    always @(posedge clk) if (SCLR) r3 <= 0; else r3 <= lfsr[23:16];
    always @(posedge clk or posedge ACLR) if (ACLR) r4 <= 0; else r4 <= lfsr[7:0] ^ lfsr[15:8];
    always @(posedge clk or negedge ACLRN) if (!ACLRN) r5 <= 0; else r5 <= lfsr[15:8] ^ lfsr[23:16];
    always @(posedge clk) if (CEA) begin if (SCLR) r7 <= 0; else r7 <= lfsr[11:4]; end
    always @(posedge clk) if (SCLR) r8 <= 0; else if (CEB) r8 <= lfsr[19:12];
    // Preset: the asynchronous clear sets these.
    always @(posedge clk or posedge ACLR) if (ACLR) pr <= 8'hff; else pr <= pr + lfsr[7:0];

    // Power-up at 1: a ring that starts with five bits set.
    reg [11:0] ring = 12'b1011_0110_1110;
    always @(posedge clk) ring <= {ring[10:0], ring[11] ^ DIN};

    reg [8:0] s = 0, t = 0;
    reg lt = 0, ge = 0;
    always @(posedge clk) begin
        s <= lfsr[7:0] + lfsr[15:8] + CIN;
        t <= lfsr[7:0] - lfsr[23:16];
        lt <= acc[47:16] < lfsr;
        ge <= acc[63:48] >= lfsr[31:16];
    end

    reg [11:0] c2 = 0;
    always @(posedge CLOCK2_50) if (CEB) c2 <= c2 + 1'b1;

    assign O[0] = ^acc[31:0];
    assign O[1] = ^acc[63:32];
    assign O[2] = ^r1 ^ ^r3;
    assign O[3] = ^r4 ^ ^r5;
    assign O[4] = ^r7 ^ ^r8;
    assign O[5] = ^pr ^ ^ring;
    assign O[6] = ^s ^ ^t;
    assign O[7] = lt ^ ge;
    assign O[8] = ^c2;
endmodule
