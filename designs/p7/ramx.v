// Phase 7 open flow (README (y)): an M20K with a write enable, byte enables,
// a read enable and its output register. The address is a 32-bit counter's
// bits 31:23 (a new address every 0.17 s); the data written is that
// counter's top byte in all four bytes.
//   SW0 up: write (WE); SW3 up: also write byte 3 (byte 0 always);
//   LED[3:0] byte 0's low bits as read back, LED[7:4] byte 3's;
//   KEY1 held: the read address stops (RE low), the LEDs freeze.
// With SW0 up the LEDs count; with SW0 down they show what was written at
// that address on the counter's previous pass (every 87 s), 0 at first.
module ramx (
    input  wire CLOCK1_50,
    input  wire KEY1, SW0, SW3,
    output wire [7:0] LED
);
    reg [31:0] c = 0;
    always @(posedge CLOCK1_50) c <= c + 1'b1;
    wire [31:0] q;
    A5_M20K #(.OUTREG(1)) m (
        .CLK(CLOCK1_50), .WE(SW0), .BE({SW3, 2'b00, 1'b1}), .RE(KEY1),
        .WADDR(c[31:23]), .WDATA({4{c[31:24]}}), .RADDR(c[31:23]), .RDATA(q));
    assign LED = {q[27:24], q[3:0]};
endmodule
