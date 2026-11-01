// Phase 6 open flow, design 3 (README (w)): carry chains across LABs. A
// 40-bit accumulator adds a 32-bit LFSR every cycle (a 41-cell chain, three
// LABs); LED[7:0] shows its top byte. KEY0 (active low) clears it.
module acc (
    input  wire CLOCK1_50,
    input  wire KEY0,
    output wire [7:0] LED
);
    reg [31:0] lfsr = 32'h1;
    reg [39:0] a = 0;
    always @(posedge CLOCK1_50) begin
        lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};
        a <= KEY0 ? a + {8'd0, lfsr} : 40'd0;
    end
    assign LED = a[39:32];
endmodule
