// Routing check design: two blocks of logic in opposite corners of the
// fabric, so their nets cross both sectors on long H and V wires. Block a is
// a 32-bit LFSR, block b a CRC-32 over it; b's result goes back to a and out
// to the HVIO pins.
module rt (
    input  wire       CLOCK1_50,
    input  wire       KEY0,
    output wire [3:0] OUT
);
    wire [31:0] la, lb;
    wire [3:0]  fb;
    lfsr a (.clk(CLOCK1_50), .rst(KEY0), .fb(fb), .q(la));
    crc  b (.clk(CLOCK1_50), .d(la), .q(lb));
    assign fb = lb[3:0] ^ lb[31:28];
    assign OUT = lb[7:4] ^ lb[15:12];
endmodule

module lfsr (
    input  wire        clk,
    input  wire        rst,
    input  wire [3:0]  fb,
    output reg  [31:0] q = 32'h1
);
    always @(posedge clk)
        if (!rst) q <= 32'h1;
        else      q <= {q[30:0], q[31] ^ q[21] ^ q[1] ^ q[0]} ^ {28'b0, fb};
endmodule

module crc (
    input  wire        clk,
    input  wire [31:0] d,
    output reg  [31:0] q = 0
);
    integer i;
    reg [31:0] c;
    always @* begin
        c = q;
        for (i = 0; i < 32; i = i + 1)
            c = {c[30:0], 1'b0} ^ ((c[31] ^ d[i]) ? 32'h04c11db7 : 32'h0);
    end
    always @(posedge clk) q <= c;
endmodule
