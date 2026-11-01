// Phase 7 open flow (README (y)): carry chains as the hardware starts and
// links them. A 30-bit counter (no start cell: the hardware carry into a
// chain's first half is 0) runs through 15 ALMs, from a LAB's top half into
// its bottom half and on into the LAB below; an adder takes SW0 as its
// carry in; a subtractor has a constant carry in.
//   LED[3:0]  the counter's top bits (LED0 toggles every 1.34 s);
//   LED[7:4]  LED[3:0] + {SW2, SW1} + SW0, or with SW3 up LED[3:0] - 3.
module arith (
    input  wire CLOCK1_50,
    input  wire SW0, SW1, SW2, SW3,
    output wire [7:0] LED
);
    reg [29:0] c = 0;
    always @(posedge CLOCK1_50) c <= c + 1'b1;
    reg [3:0] s = 0, d = 0;
    always @(posedge CLOCK1_50) begin
        s <= c[29:26] + {SW2, SW1} + SW0;
        d <= c[29:26] - 4'd3;
    end
    assign LED = {SW3 ? d : s, c[29:26]};
endmodule
