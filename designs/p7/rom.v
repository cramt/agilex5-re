// Phase 7 open flow (README (y)): an M20K as a ROM, its contents in the
// image (writes off through the write enable's inverter). Word k holds a
// light running back and forth over LED[7:0]; the read address steps every
// 2^22 cycles of CLOCK1_50 (12 Hz), so the light sweeps 8 LEDs in 0.67 s.
module rom (
    input  wire CLOCK1_50,
    output wire [7:0] LED
);
    function [31:0] word(input integer k);
        integer p;
        begin
            p = k % 14;
            p = p < 8 ? p : 14 - p;
            word = {k[7:0], ~k[7:0], 8'h5a, 8'd1 << p};
        end
    endfunction
    function [16383:0] contents(input integer dummy);
        integer k;
        begin
            contents = 0;
            for (k = 0; k < 512; k = k + 1) contents[32 * k +: 32] = word(k);
        end
    endfunction
    reg [30:0] c = 0;
    always @(posedge CLOCK1_50) c <= c + 1'b1;
    wire [31:0] q;
    A5_M20K #(.INIT(contents(0))) m (
        .CLK(CLOCK1_50), .WE(1'b0), .BE(4'b1111), .RE(1'b1),
        .WADDR(9'd0), .WDATA(32'd0), .RADDR(c[30:22]), .RDATA(q));
    assign LED = q[7:0];
endmodule
