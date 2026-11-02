// Phase 10 oracle design (README (z11)): DSP blocks in the modes a5pnr maps,
// each fed from its own bits of a 64-bit LFSR so a decoded image shows which
// DSP pin carries which operand bit: an 18x18 signed multiply with input and
// output registers, an 18x18 unsigned one fed straight from the LFSR, a 27x27
// unsigned multiply, a 27x27 multiply-accumulate with an enable and a
// synchronous clear, and a sum of two 18x18 products. Every product is
// XOR-reduced to an output pin and its low bits sent to LEDs.
module dq (
    input  wire CLOCK1_50,
    input  wire EN, CLR,
    output wire [4:0] O,
    output wire [7:0] LED
);
    wire clk = CLOCK1_50;
    reg [63:0] s = 64'h1;
    always @(posedge clk) s <= {s[62:0], s[63] ^ s[62] ^ s[60] ^ s[59]};

    // Signed 18x18, registered operands and product.
    reg signed [17:0] a1, b1;
    reg signed [35:0] p1;
    always @(posedge clk) begin a1 <= s[17:0]; b1 <= s[35:18]; p1 <= a1 * b1; end

    // Unsigned 18x18 from the LFSR, product registered.
    reg [35:0] p2;
    always @(posedge clk) p2 <= s[53:36] * {s[63:54], s[7:0]};

    // Unsigned 27x27, product registered.
    reg [53:0] p3;
    always @(posedge clk) p3 <= s[26:0] * {s[63:37]};

    // 27x27 multiply-accumulate, enable and synchronous clear.
    reg [63:0] acc;
    always @(posedge clk) if (CLR) acc <= 0; else if (EN) acc <= acc + s[40:14] * s[59:33];

    // Sum of two 18x18 products.
    reg [36:0] p5;
    always @(posedge clk) p5 <= s[44:27] * s[62:45] + s[19:2] * s[37:20];

    assign O = {^p5, ^acc, ^p3, ^p2, ^p1};
    assign LED = p1[7:0] ^ p2[7:0] ^ p3[7:0] ^ acc[7:0] ^ p5[7:0];
endmodule
