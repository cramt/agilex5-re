// Phase 6 open flow, design 3 (README (w)): carry chains across LABs. A
// 56-bit accumulator adds a 32-bit LFSR (mean 2^31) every cycle: a 57-cell
// chain over three LABs. LED[7:0] shows bits 55..48, so LED7 changes about
// every 2^24 cycles (0.3 s at 50 MHz) and the lower LEDs faster, in an
// irregular pattern. KEY0 (active low) clears it.
module acc (
    input  wire CLOCK1_50,
    input  wire KEY0,
    output wire [7:0] LED
);
    reg [31:0] lfsr = 32'h1;
    reg [55:0] a = 0;
    always @(posedge CLOCK1_50) begin
        lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};
        a <= KEY0 ? a + {24'd0, lfsr} : 56'd0;
    end
    assign LED = a[55:48];
endmodule
