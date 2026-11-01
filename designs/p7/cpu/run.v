// RTL check of the program (README (z2)): from power-up, KEY0 up, SW0 up
// (fast); prints each LED value the program stores and its cycle.
`timescale 1ns/1ps
module run;
    reg clk = 0, sw0 = 1;
    wire [7:0] led;
    cpu dut(.CLOCK1_50(clk), .KEY0(1'b1), .SW0(sw0), .LED(led));
    integer cyc = 0;
    reg [7:0] last = 0;
    always #10 clk = ~clk;
    always @(posedge clk) begin
        cyc <= cyc + 1;
        if (led !== last) begin
            $display("cycle %0d: LED %0d", cyc, led);
            last <= led;
        end
        if (cyc == 2000) $finish;
    end
endmodule
