// Phase 5 clocks (README (r)): every clock-capable HVIO pin of the DE25-Nano
// (the four xx4_u2 pins of each of the 8 banks) and CLOCK0_50 (IO96) each clock
// a 2-bit counter with a clock enable; CLOCK2_50 asks for a regional clock.
// IO registers: two inputs with FAST_INPUT_REGISTER and one output with
// FAST_OUTPUT_REGISTER, as the DE25-Nano SDRAM pins use them.
module ck (
    input  wire [31:0] CK,
    input  wire CLOCK0_50,
    input  wire EN,
    input  wire [1:0] DQ,
    output wire DQO,
    output wire LED0,
    output wire O
);
    reg [1:0] c [0:32];
    genvar i;
    generate for (i = 0; i < 32; i = i + 1) begin : g
        always @(posedge CK[i]) if (EN) c[i] <= c[i] + 1'b1;
    end endgenerate
    always @(posedge CLOCK0_50) if (EN) c[32] <= c[32] + 1'b1;
    // CK[15] is CLOCK1_50 (V16).
    reg [1:0] dq; reg dqo;
    always @(posedge CK[15]) begin dq <= DQ; dqo <= dq[0] ^ dq[1]; end
    assign DQO = dqo;
    assign LED0 = c[32][1];
    assign O = c[0][1] ^ c[1][1] ^ c[2][1] ^ c[3][1] ^ c[4][1] ^ c[5][1] ^ c[6][1] ^ c[7][1] ^ c[8][1] ^ c[9][1] ^ c[10][1] ^ c[11][1] ^ c[12][1] ^ c[13][1] ^ c[14][1] ^ c[15][1] ^ c[16][1] ^ c[17][1] ^ c[18][1] ^ c[19][1] ^ c[20][1] ^ c[21][1] ^ c[22][1] ^ c[23][1] ^ c[24][1] ^ c[25][1] ^ c[26][1] ^ c[27][1] ^ c[28][1] ^ c[29][1] ^ c[30][1] ^ c[31][1];
endmodule
