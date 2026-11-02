// $mul → A5_MUL (README (z11)), before synth: a product of two operands of
// up to 27 bits, both signed or both unsigned, in one DSP block, 18x18 when
// both fit; the operands extended to the multiplier's width (so every pin
// carries a known signal), the product cut or extended to Y.
(* techmap_celltype = "$mul" *)
module _80_a5_mul (A, B, Y);
parameter A_SIGNED = 0;
parameter B_SIGNED = 0;
parameter A_WIDTH = 1;
parameter B_WIDTH = 1;
parameter Y_WIDTH = 1;
input [A_WIDTH-1:0] A;
input [B_WIDTH-1:0] B;
output [Y_WIDTH-1:0] Y;
localparam S = A_SIGNED && B_SIGNED;
localparam WIDE = A_WIDTH > 18 || B_WIDTH > 18;
localparam N = WIDE ? 27 : 18;
generate
if (A_SIGNED != B_SIGNED || A_WIDTH > 27 || B_WIDTH > 27 || A_WIDTH * B_WIDTH < 36 || A_WIDTH < 3 || B_WIDTH < 3) begin
    // Mixed signs, too wide, or small enough for LUTs.
    wire _TECHMAP_FAIL_ = 1;
end else begin
    wire [26:0] a, b;
    wire [53:0] p;
    \$pos #(.A_SIGNED(S), .A_WIDTH(A_WIDTH), .Y_WIDTH(N)) ea (.A(A), .Y(a[N-1:0]));
    \$pos #(.A_SIGNED(S), .A_WIDTH(B_WIDTH), .Y_WIDTH(N)) eb (.A(B), .Y(b[N-1:0]));
    if (N < 27) begin
        assign a[26:N] = 0;
        assign b[26:N] = 0;
    end
    A5_MUL #(.SIGNED(S), .WIDE(WIDE)) _TECHMAP_REPLACE_ (.A(a), .B(b), .P(p));
    \$pos #(.A_SIGNED(S), .A_WIDTH(2 * N), .Y_WIDTH(Y_WIDTH)) ey (.A(p[2*N-1:0]), .Y(Y));
end
endgenerate
endmodule
