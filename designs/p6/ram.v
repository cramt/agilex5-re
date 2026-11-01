// Phase 6 open flow, design 4 (README (w)): one M20K as a 512 x 32 simple
// dual-port RAM (tools/a5pnr/yosys/cells.v A5_M20K). A counter writes every
// address with its own number (repeated across the word) every 512 cycles;
// the read address steps every 2^23 cycles (0.17 s at 50 MHz), so LED[7:0]
// shows the RAM's copy of the read address: a binary count at about 6 Hz.
module ram (
    input  wire CLOCK1_50,
    output wire [7:0] LED
);
    reg [31:0] c = 0;
    always @(posedge CLOCK1_50) c <= c + 1'b1;
    wire [31:0] q;
    A5_M20K m (.CLK(CLOCK1_50), .WE(1'b1), .BE(4'b1111), .RE(1'b1), .WADDR(c[8:0]), .WDATA({c[4:0], c[8:0], c[8:0], c[8:0]}),
               .RADDR(c[31:23]), .RDATA(q));
    assign LED = q[7:0];
endmodule
