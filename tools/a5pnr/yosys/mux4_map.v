// $_MUX4_ (muxcover) as one 6-input LUT, Y = {D, C, B, A}[{T, S}]: a 4:1 mux
// fills a 6-LUT, where abc alone built a register file's read muxes from
// 2:1 muxes first (README (z3)).
module \$_MUX4_ (input A, B, C, D, S, T, output Y);
    \$lut #(.WIDTH(6), .LUT(64'hFF00F0F0CCCCAAAA)) _TECHMAP_REPLACE_ (.A({T, S, D, C, B, A}), .Y(Y));
endmodule
