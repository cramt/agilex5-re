// $alu → a chain of A5_ARITH (README (w)). Bit i adds AA[i] and BB[i]
// (B inverted when BI): p = A ^ B ^ C with C = BI, g = A & (B ^ C). The
// chain starts with a cell whose carry out is CI (p = 0, g = A = CI): the
// hardware chain has no free carry-in.
(* techmap_celltype = "$alu" *)
module _80_a5_alu (A, B, CI, BI, X, Y, CO);
parameter A_SIGNED = 0;
parameter B_SIGNED = 0;
parameter A_WIDTH = 1;
parameter B_WIDTH = 1;
parameter Y_WIDTH = 1;
(* force_downto *) input [A_WIDTH-1:0] A;
(* force_downto *) input [B_WIDTH-1:0] B;
input CI, BI;
(* force_downto *) output [Y_WIDTH-1:0] X, Y, CO;
(* force_downto *) wire [Y_WIDTH-1:0] AA, BB;
\$pos #(.A_SIGNED(A_SIGNED), .A_WIDTH(A_WIDTH), .Y_WIDTH(Y_WIDTH)) a_conv (.A(A), .Y(AA));
\$pos #(.A_SIGNED(B_SIGNED), .A_WIDTH(B_WIDTH), .Y_WIDTH(Y_WIDTH)) b_conv (.A(B), .Y(BB));
wire [Y_WIDTH:0] C;
A5_ARITH #(.P(16'h0000), .G(16'hAAAA)) start (.A(CI), .B(1'b0), .C(1'b0), .D(1'b0), .CI(1'b0), .CO(C[0]));
genvar i;
generate for (i = 0; i < Y_WIDTH; i = i + 1) begin : slice
    // p over {D, C, B, A} = A ^ B ^ C; g = A & (B ^ C).
    A5_ARITH #(.P(16'h9696), .G(16'h2828)) cell (.A(AA[i]), .B(BB[i]), .C(BI), .D(1'b0), .CI(C[i]), .SO(Y[i]), .CO(C[i+1]));
    wire bb = BB[i] ^ BI;
    assign CO[i] = (AA[i] & bb) | ((AA[i] ^ bb) & C[i]);
end endgenerate
assign X = AA ^ (BB ^ {Y_WIDTH{BI}});
endmodule
