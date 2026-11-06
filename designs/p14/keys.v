// Keys and switches in the whole-row language (README, "Self-tests"); needs a
// person at the bench. KEYs are active low (pressed = 0), SWs up = 1, LEDs
// active low. First match wins:
//   KEY0 held            row steadily lit
//   KEY1 held            row blinks at 4 Hz
//   SW0 up               0.5 Hz
//   SW1 up               1 Hz
//   SW2 up               2 Hz
//   SW3 up               3 Hz
//   otherwise            row dark
module keys (
    input  wire CLOCK1_50,
    input  wire KEY0, KEY1,
    input  wire [3:0] SW,
    output reg  [7:0] LED = 8'hff
);
    wire clk = CLOCK1_50;
    // Two flops on every input: the keys and switches are asynchronous.
    reg [5:0] s1 = 6'h03, s2 = 6'h03;
    always @(posedge clk) begin s1 <= {SW, KEY1, KEY0}; s2 <= s1; end
    wire k0 = !s2[0], k1 = !s2[1];
    wire [3:0] sw = s2[5:2];
    // Half periods in CLOCK1_50 cycles.
    wire [25:0] half = k1    ? 26'd6_250_000 :
                       sw[0] ? 26'd50_000_000 :
                       sw[1] ? 26'd25_000_000 :
                       sw[2] ? 26'd12_500_000 :
                               26'd8_333_333;
    wire blink = k1 || |sw;
    reg [25:0] t = 0;
    reg ph = 0;
    always @(posedge clk) begin
        if (t >= half - 1'b1) begin t <= 0; ph <= ~ph; end
        else t <= t + 1'b1;
        LED <= k0 ? 8'h00 : blink ? {8{ph}} : 8'hff;
    end
endmodule
