// LED[0] toggles at ~1.5 Hz from the 50 MHz 3.3 V clock; KEY[0] (active low) holds it in reset.
module blinky (
    input  wire CLOCK1_50,
    input  wire KEY0,
    output wire LED0
);
    reg [24:0] count = 0;
    always @(posedge CLOCK1_50)
        if (!KEY0) count <= 0;
        else       count <= count + 1'b1;
    assign LED0 = count[24];
endmodule
