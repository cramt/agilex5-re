// Phase 6 open flow, design 2 (README (w)): a 27-bit counter on CLOCK1_50
// shown on LED[7:0] (its top 8 bits). KEY0 (active low) holds the counter
// at zero; KEY1 (active low) inverts the LEDs; SW0 reverses the LED order.
module leds (
    input  wire CLOCK1_50,
    input  wire KEY0, KEY1,
    input  wire SW0,
    output wire [7:0] LED
);
    reg [26:0] c = 0;
    always @(posedge CLOCK1_50) c <= KEY0 ? c + 1'b1 : 27'd0;
    wire [7:0] top = c[26:19];
    wire [7:0] rev = {top[0], top[1], top[2], top[3], top[4], top[5], top[6], top[7]};
    assign LED = (SW0 ? rev : top) ^ {8{~KEY1}};
endmodule
