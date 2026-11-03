// Phase 10 oracle design (README (z11)): DSP blocks with every product bit
// observable. Operands come straight from a 64-bit LFSR (each bit its own
// flop), products go into the DSP's output register, and each product bit
// then into a fabric flop through a LUT of its own (q[k] = p[k] ^ s[k]), so
// a decoded image shows which DSP pin carries which bit: 18x18 signed and
// unsigned, 27x27 signed and unsigned, and a sum of two 18x18 products.
module dq2 (
    input  wire CLOCK1_50,
    output wire [4:0] O
);
    wire clk = CLOCK1_50;
    reg [63:0] s = 64'h1;
    always @(posedge clk) s <= {s[62:0], s[63] ^ s[62] ^ s[60] ^ s[59]};

    reg signed [35:0] p1 = 0;
    reg [35:0] p2 = 0;
    reg [53:0] p3 = 0;
    reg signed [53:0] p4 = 0;
    reg [36:0] p5 = 0;
    always @(posedge clk) begin
        p1 <= $signed(s[17:0]) * $signed(s[35:18]);
        p2 <= s[53:36] * {s[63:54], s[7:0]};
        p3 <= s[26:0] * s[63:37];
        p4 <= $signed(s[44:18]) * $signed({s[8:0], s[63:46]});
        p5 <= s[44:27] * s[62:45] + s[19:2] * s[37:20];
    end

    reg [35:0] q1 = 0, q2 = 0;
    reg [53:0] q3 = 0, q4 = 0;
    reg [36:0] q5 = 0;
    always @(posedge clk) begin
        q1 <= p1 ^ s[35:0];
        q2 <= p2 ^ s[63:28];
        q3 <= p3 ^ s[53:0];
        q4 <= p4 ^ s[63:10];
        q5 <= p5 ^ s[36:0];
    end
    assign O = {^q5, ^q4, ^q3, ^q2, ^q1};
endmodule
